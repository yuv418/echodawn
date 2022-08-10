@0xef1e02eb2765bbab;



interface EdcsProtocol {
    struct EdcsMessage {
        messageType @0 :EdcsMessageType;
        payload :union {
            setupStreamParams @1 :EdcsStreamParams;
            updateStreamParams @2  :EdcsStreamParams;
        }
    }

    struct EdcsStreamParams {
        framerate @0 :Int32;
        bitrate @1 :Int32;
    }

    enum EdcsMessageType {
        setupStream @0;
        startStream @1;
        closeStream @2;
        updateStream @3;
    }

    enum EdcsStatus {
        ok @0;
        err @1;
    }

    struct EdcsResponse {
        status @0 :EdcsStatus;
    }

}
