use cxx::CxxString;

#[cxx::bridge(namespace = "edc_decoder")]
pub mod decoder_bridge {

    unsafe extern "C++" {
        include!("edc/src/edc_decoder/cpp_decoder/inc/decoder.h");

        pub type EdcDecoder;
        pub type AVFramePublic;
        pub fn new_edc_decoder(
            sdp_file_path: &str,
            width: u32,
            height: u32,
        ) -> UniquePtr<EdcDecoder>;
        pub fn fetch_ring_frame(self: &EdcDecoder) -> *mut AVFramePublic;
        pub fn start_decoding(self: Pin<&mut EdcDecoder>);
    }
}

#[cfg(test)]
pub mod test_decoder_bridge {
    use cxx::{let_cxx_string, CxxString, UniquePtr};

    use super::decoder_bridge;

    #[test]
    pub fn init_decoder_bridge() {
        let decoder = decoder_bridge::new_edc_decoder("bla");
        if let Some(pt) = decoder.as_ref() {
            println!("\naddress {:p}", pt);
        } else {
            println!("\ndecoder was null");
        }
        assert_eq!(decoder.is_null(), false);
    }
}
