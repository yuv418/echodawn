use std::{
    ptr::{null, null_mut},
    rc::Rc,
};

use anyhow::anyhow;
use cxx::UniquePtr;
use ffmpeg_sys_next::AVFrame;
use glow::{HasContext, NativeTexture, NativeUniformLocation, NativeVertexArray};
use glutin::{
    event::WindowEvent,
    event_loop::ControlFlow,
    window::{Window, WindowId},
};
use log::trace;

use super::{mpv::MPVEvent, video_decoder::VideoDecoder};
use crate::edc_decoder::decoder_bridge::{self, EdcDecoder};

// I don't actually know OpenGL. The OpenGL calls in this file are pretty much copied from
// https://gist.github.com/Beyley/eb83d9d5f138dfca36c284b7831333b5.

pub struct FFmpegCtx {
    decoder: UniquePtr<EdcDecoder>,
    width: u32,
    height: u32,
    texture: NativeTexture,
    vao: NativeVertexArray,
    uniform_mvp_matrix: NativeUniformLocation,
    uniform_frame_tex: NativeUniformLocation,
    attribs_vertices: u32,
    attribs_tex_coords: u32,
}

impl std::fmt::Debug for FFmpegCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FFmpegCtx")
            .field("decoder -> {}", &"CPP impl")
            .finish()
    }
}

impl VideoDecoder for FFmpegCtx {
    fn new(
        gl: Rc<glow::Context>,
        window: &Window,
        width: u32,
        height: u32,
        debug: bool,
        sdp: String,
    ) -> anyhow::Result<Box<dyn VideoDecoder>> {
        let mut decoder = decoder_bridge::new_edc_decoder(&sdp, width, height);
        trace!("decoder pointer is {:p}", decoder.as_mut().unwrap());
        let (
            texture,
            vao,
            prgm,
            attribs_vertices,
            attribs_tex_coords,
            uniform_mvp_matrix,
            uniform_frame_tex,
        ) = unsafe {
            let vertex_shader_source = r#"
                #version 150
                in vec3 vertex;
                in vec2 texCoord0;
                uniform mat4 mvpMatrix;
                out vec2 texCoord;
                void main() {
                    gl_Position = mvpMatrix * vec4(vertex, 1.0);
                    texCoord = texCoord0;
                }"#;
            let frag_shader_source = r#"
                #version 150
                uniform sampler2D frameTex;
                in vec2 texCoord;
                out vec4 fragColor;

                void main() {
                    fragColor = texture(frameTex, texCoord);
                }
                "#;

            let shader_src = [
                (glow::VERTEX_SHADER, vertex_shader_source),
                (glow::FRAGMENT_SHADER, frag_shader_source),
            ];
            gl.clear_color(0.0, 0.0, 0.0, 0.0);
            gl.enable(glow::TEXTURE_2D);
            gl.disable(glow::MULTISAMPLE);
            let prgm = gl
                .create_program()
                .expect("Failed to create ffmpeg GL program");
            let mut shaders = vec![];
            for (shader_ty, shader_code) in shader_src {
                let shader = gl
                    .create_shader(shader_ty)
                    .expect("Failed to create shader");
                gl.shader_source(shader, shader_code);
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    panic!("{}", gl.get_shader_info_log(shader));
                }
                gl.attach_shader(prgm, shader);
                shaders.push(shader);
            }
            gl.link_program(prgm);
            if !gl.get_program_link_status(prgm) {
                panic!("{}", gl.get_program_info_log(prgm));
            }
            /*for shader in shaders {
                gl.detach_shader(prgm, shader);
                gl.delete_shader(shader);
            }*/
            let uniform_mvp_matrix = gl
                .get_uniform_location(prgm, "mvpMatrix")
                .expect("Failed to get mvpMatrix loc");
            let uniform_frame_tex = gl
                .get_uniform_location(prgm, "frameTex")
                .expect("Failed to get frameTex loc");
            let attribs_vertices = gl.get_attrib_location(prgm, "vertex").unwrap();
            let attribs_tex_coords = gl.get_attrib_location(prgm, "texCoord0").unwrap();

            gl.use_program(Some(prgm));

            let vao = gl.create_vertex_array().expect("failed to create vao");
            gl.bind_vertex_array(Some(vao));

            let buffer = gl.create_buffer().expect("failed to create gl buffer");
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(buffer));

            // quad buffer
            let quad2 = [
                -1.0f32, 1.0f32, 0.0f32, 0.0f32, 0.0f32, -1.0f32, -1.0f32, 0.0f32, 0.0f32, 1.0f32,
                1.0f32, -1.0f32, 0.0f32, 1.0f32, 1.0f32, 1.0f32, 1.0f32, 0.0f32, 1.0f32, 0.0f32,
            ];
            trace!("size of quad is {}", quad2.len());
            let quad_u8 = std::slice::from_raw_parts(
                quad2.as_ptr() as *const u8,
                quad2.len() * core::mem::size_of::<f32>(),
            );
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, &quad_u8, glow::STATIC_DRAW);
            gl.vertex_attrib_pointer_f32(attribs_vertices, 3, glow::FLOAT, false, 20, 0);
            gl.enable_vertex_attrib_array(attribs_vertices);
            gl.vertex_attrib_pointer_f32(attribs_tex_coords, 2, glow::FLOAT, false, 20, 12);
            gl.enable_vertex_attrib_array(attribs_tex_coords);

            // different buffer...
            let elem_buf = gl.create_buffer().expect("Failed to make elbuf");
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(elem_buf));
            gl.buffer_data_u8_slice(
                glow::ELEMENT_ARRAY_BUFFER,
                &[0, 1, 2, 0, 2, 3],
                glow::STATIC_DRAW,
            );
            gl.bind_vertex_array(None);

            gl.active_texture(glow::TEXTURE0);
            let texture = gl.create_texture().expect("failed to create gl texture");
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::REPEAT as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::REPEAT as i32);
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGB as i32,
                1920 as i32,
                1080 as i32,
                0,
                glow::RGB,
                glow::UNSIGNED_BYTE,
                None,
            );
            gl.uniform_1_i32(Some(&uniform_frame_tex), 0);

            let mvp = nalgebra_glm::ortho(-1.0, 1.0, -1.0, 1.0, -1.0, 1.0);
            gl.uniform_matrix_4_f32_slice(
                Some(&uniform_mvp_matrix),
                false,
                nalgebra_glm::value_ptr(&mvp),
            );

            (
                texture,
                vao,
                prgm,
                attribs_vertices,
                attribs_tex_coords,
                uniform_mvp_matrix,
                uniform_frame_tex,
            )
        };

        Ok(Box::new(Self {
            decoder,
            width,
            height,
            texture,
            vao,
            attribs_vertices,
            attribs_tex_coords,
            uniform_mvp_matrix,
            uniform_frame_tex,
        }))
    }

    fn paint(&mut self, gl: Rc<glow::Context>, window: &Window) {
        let frame = self.decoder.fetch_ring_frame();
        if frame.is_null() {
            return;
        }
        unsafe {
            // I love overriding the Rust type system /s
            let frame: *mut AVFrame = std::mem::transmute(frame);
            let frame_length = ffmpeg_sys_next::av_image_get_buffer_size(
                ffmpeg_sys_next::AVPixelFormat::AV_PIX_FMT_RGB24,
                (*frame).width,
                (*frame).height,
                32,
            );
            let pixels_slice = std::slice::from_raw_parts((*frame).data[0], frame_length as usize);
            /*l.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, 0);
            gl.pixel_store_i32(glow::UNPACK_SKIP_PIXELS, 0);
            gl.pixel_store_i32(glow::UNPACK_SKIP_ROWS, 0);*/
            gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                0,
                0,
                (*frame).width as i32,
                (*frame).height as i32,
                glow::RGB,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(pixels_slice),
            );
            gl.clear(glow::COLOR_BUFFER_BIT);
            // the problem is probably on the next line or the next next line.
            gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
            gl.bind_vertex_array(Some(self.vao));
            gl.draw_elements(glow::TRIANGLES, 6, glow::UNSIGNED_BYTE, 0);
            gl.bind_vertex_array(None);
            libc::free((*frame).data[0] as *mut libc::c_void);
            libc::free(frame as *mut libc::c_void);
        }
    }

    fn handle_window_event(&self, _window_id: WindowId, event: WindowEvent) {}

    fn handle_user_event(&self, window: &Window, _ctrl_flow: &ControlFlow, event: &MPVEvent) {}

    fn needs_evloop_proxy(&mut self) -> bool {
        false
    }

    fn give_evloop_proxy(
        &mut self,
        evloop_proxy: std::rc::Rc<glutin::event_loop::EventLoopProxy<MPVEvent>>,
    ) -> bool {
        true
    }

    fn start_decoding(&mut self) {
        // Start the stream.
        self.decoder.as_mut().unwrap().start_decoding();
    }
}
