use super::Av1Profile;

const OBU_SEQUENCE_HEADER: u8 = 1;
const OBU_TEMPORAL_DELIMITER: u8 = 2;
const OBU_FRAME_HEADER: u8 = 3;
const OBU_TILE_GROUP: u8 = 4;
const OBU_METADATA: u8 = 5;
const OBU_FRAME: u8 = 6;
const OBU_REDUNDANT_FRAME_HEADER: u8 = 7;
const OBU_TILE_LIST: u8 = 8;
const OBU_PADDING: u8 = 15;

struct ObuExtensionHeader {
    temporal_id: u32,
    spatial_id: u32,
}

struct SequenceHeaderTimingInfo {
    decoder_model_info: Optional<SequenceHeaderDecoderModelInfo>,
}

struct SequenceHeaderObu {
    seq_profile: Av1Profile,
    still_picture: bool,
    seq_level_idx: [u8; ],
    timing_info: Option<SequenceHeaderTimingInfo>,
    frame_width: u16,
    frame_height: u16,
    use_128x128_superblock: bool,
    enable_filter_intra: bool,
    enable_intra_edge_filter: bool,
    enable_interintra_compound: bool,
    enable_masked_compound: bool,
    enable_warped_motion: bool,
    enable_dual_filter: bool,
    enable_order_hint: bool,
    enable_jnt_comp: bool,
    enable_ref_frame_mvs: bool,
    seq_force_screen_content_tools: bool,
    seq_force_integer_mv: bool,
    order_hint_bits_minus_1: bool,
}


enum ObuType {
    SequenceHeader(SequenceHeaderObu),
}

struct Obu<'a> {
    extension_header: Option<ObuExtensionHeader>, // obu_extension_flag + obu_extension_flag

    buf: &'a [u8], // starts at OBU payload
}

impl<'a> Obu<'a> {
    pub fn from_buf(buf: &'a [u8]) -> Self {
        Self { buf }
    }
}
