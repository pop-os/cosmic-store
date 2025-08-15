#[derive(Clone, Debug)]
pub struct GStreamerCodec {
    pub version: String,
    pub application: String,
    pub description: String,
    pub type_name: String,
}

impl GStreamerCodec {
    pub fn parse(input: &str) -> Option<Self> {
        // Input looks like gstreamer|1.0|cosmic-player|H.264 (Main Profile) decoder|decoder-video/x-h264, level=(string)3.1, profile=(string)main
        let mut parts = input.split('|');
        let gstreamer = parts.next()?;
        if gstreamer != "gstreamer" {
            return None;
        }
        let version = parts.next()?.to_string();
        let application = parts.next()?.to_string();
        let description = parts.next()?.to_string();

        let type_string = parts.next()?;
        let mut type_parts = type_string.split(", ");
        let type_name = type_parts.next()?.to_string();
        //TODO: handle remainder of type_parts and parts?

        Some(Self {
            version,
            application,
            description,
            type_name,
        })
    }
}
