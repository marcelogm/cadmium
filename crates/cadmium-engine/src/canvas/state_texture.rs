#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StateTexture {
    WetPig0,
    WetPig1,
    WetVol,
    DryRefl0,
    DryRefl1,
    DryHeight,
    DryTopPig0,
    DryTopPig1,
    Display,
}

impl StateTexture {
    pub const ALL: [StateTexture; 9] = [
        StateTexture::WetPig0,
        StateTexture::WetPig1,
        StateTexture::WetVol,
        StateTexture::DryRefl0,
        StateTexture::DryRefl1,
        StateTexture::DryHeight,
        StateTexture::DryTopPig0,
        StateTexture::DryTopPig1,
        StateTexture::Display,
    ];

    pub const PAINT_STATE: [StateTexture; 8] = [
        StateTexture::WetPig0,
        StateTexture::WetPig1,
        StateTexture::WetVol,
        StateTexture::DryRefl0,
        StateTexture::DryRefl1,
        StateTexture::DryHeight,
        StateTexture::DryTopPig0,
        StateTexture::DryTopPig1,
    ];

    pub fn format(self) -> wgpu::TextureFormat {
        match self {
            StateTexture::WetVol | StateTexture::DryHeight => wgpu::TextureFormat::R32Float,
            _ => wgpu::TextureFormat::Rgba16Float,
        }
    }

    pub fn channels(self) -> u32 {
        match self.format() {
            wgpu::TextureFormat::R32Float => 1,
            _ => 4,
        }
    }

    pub fn bytes_per_texel(self) -> u32 {
        self.format()
            .block_copy_size(None)
            .expect("state textures are uncompressed colour formats")
    }

    pub fn label(self) -> &'static str {
        match self {
            StateTexture::WetPig0 => "wet_pig0",
            StateTexture::WetPig1 => "wet_pig1",
            StateTexture::WetVol => "wet_vol",
            StateTexture::DryRefl0 => "dry_refl0",
            StateTexture::DryRefl1 => "dry_refl1",
            StateTexture::DryHeight => "dry_height",
            StateTexture::DryTopPig0 => "dry_top_pig0",
            StateTexture::DryTopPig1 => "dry_top_pig1",
            StateTexture::Display => "display",
        }
    }

    pub(super) fn index(self) -> usize {
        self as usize
    }
}
