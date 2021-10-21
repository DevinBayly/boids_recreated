pub mod conversion_tools {
    use naga::back::spv;
    use naga::front::glsl::{Options, Parser};
    use naga::valid;
    use naga::ShaderStage;
    use std::fs;
    use std::io::prelude::*;
    pub fn convert_src(fname: &str) -> Vec<u32> {
        let source = String::from_utf8(fs::read(fname).expect("couldn't load shader file code ")).expect(
            "string convert failed"
        );
        let mut parser = Parser::default();
        let mut op = Options::from(ShaderStage::Vertex);
        if fname.contains("frag") {
            op = Options::from(ShaderStage::Fragment);
        }
        let module = parser
            .parse(&op, &source)
            .unwrap();

        let info = valid::Validator::new(valid::ValidationFlags::all(), valid::Capabilities::all())
            .validate(&module)
            .expect("validation failed");

        let mut words = vec![];
        let mut writer = spv::Writer::new(&spv::Options::default()).unwrap();
        writer.write(&module, &info, None, &mut words).unwrap();
        writer.get_capabilities_used().clone();
        return words;
    }
    fn convert_to_bytes(ivec: Vec<u32>) -> Vec<u8> {
        let mut res:Vec<u8> = vec!();
        for v in ivec {
            res.extend(v.to_be_bytes());
        }
        return res;

    }
}
