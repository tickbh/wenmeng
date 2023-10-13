

pub struct Consts;

impl Consts {
    /// 原始加密信息, 如果收到头header为brotli则+COMPRESS_METHOD_BROTLI则归为0, 则原始数据不处理
    pub const COMPRESS_METHOD_ORIGIN_BROTLI: i8 = -3;
    pub const COMPRESS_METHOD_ORIGIN_DEFLATE: i8 = -2;
    pub const COMPRESS_METHOD_ORIGIN_GZIP: i8 = -1;
    pub const COMPRESS_METHOD_NONE: i8 = 0;
    pub const COMPRESS_METHOD_GZIP: i8 = 1;
    pub const COMPRESS_METHOD_DEFLATE: i8 = 2;
    pub const COMPRESS_METHOD_BROTLI: i8 = 3;
}