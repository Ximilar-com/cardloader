//! The vision language models cardloader can download and run on the device.
//!
//! Files are pinned to a repository revision and checked against their SHA-256, so what runs is exactly what
//! was tested. The weights are never bundled: each user downloads them from Hugging Face after accepting the
//! model's license.

pub struct ModelFile {
    pub name: &'static str,
    pub url: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

pub struct ModelSpec {
    pub id: &'static str,
    pub name: &'static str,
    /// "Fast", "Balanced", "Accurate": where the model sits among the others.
    pub tier: &'static str,
    /// Parameter count as the model card states it, e.g. "450M".
    pub params: &'static str,
    pub description: &'static str,
    /// Memory the loaded model needs, roughly (MB).
    pub memory_mb: u32,
    pub license_name: &'static str,
    pub license_url: &'static str,
    pub homepage: &'static str,
    /// The language model, then its vision projector (mmproj).
    pub model: ModelFile,
    pub mmproj: ModelFile,
}

impl ModelSpec {
    pub fn files(&self) -> [&ModelFile; 2] {
        [&self.model, &self.mmproj]
    }

    pub fn size(&self) -> u64 {
        self.model.size + self.mmproj.size
    }
}

pub const DEFAULT_MODEL: &str = "lfm2.5-vl-3b";

const LFM_LICENSE: &str = "LFM Open License v1.0";
const LFM_LICENSE_URL: &str = "https://www.liquid.ai/lfm-license";

/// A file of a Hugging Face repository at a pinned revision.
macro_rules! hf {
    ($repo:literal @ $rev:literal / $file:literal) => {
        concat!(
            "https://huggingface.co/",
            $repo,
            "/resolve/",
            $rev,
            "/",
            $file
        )
    };
}

/// The default first, then smallest first: the order the Settings page and the pickers list them in.
pub const MODELS: &[ModelSpec] = &[
    ModelSpec {
        id: DEFAULT_MODEL,
        name: "LFM2.5-VL 3B",
        tier: "Accurate",
        params: "3B",
        description: "Liquid AI's largest LFM2.5 vision model (Q4_K_M): the most reliable names and numbers, and the \
                      slowest. Best on a computer with 8 GB of memory or more.",
        memory_mb: 2_500,
        license_name: LFM_LICENSE,
        license_url: LFM_LICENSE_URL,
        homepage: "https://huggingface.co/LiquidAI/LFM2.5-VL-3B-GGUF",
        model: ModelFile {
            name: "LFM2.5-VL-3B-Q4_K_M.gguf",
            url: hf!("LiquidAI/LFM2.5-VL-3B-GGUF" @ "6f730e9a2c454e8af9adc29db58e638e01e5957f" / "LFM2.5-VL-3B-Q4_K_M.gguf"),
            size: 1_674_455_072,
            sha256: "2436cf4bbac9a16e5dfc7799a140e583c2bc15958f4b24a391f8a8b10ecb8884",
        },
        mmproj: ModelFile {
            name: "mmproj-LFM2.5-VL-3B-Q8_0.gguf",
            url: hf!("LiquidAI/LFM2.5-VL-3B-GGUF" @ "6f730e9a2c454e8af9adc29db58e638e01e5957f" / "mmproj-LFM2.5-VL-3B-Q8_0.gguf"),
            size: 583_109_984,
            sha256: "ecbbe7097f696dba67172738d79c9f01132cdb6c0b457606315e268df3d67e64",
        },
    },
    ModelSpec {
        id: "lfm2.5-vl-450m",
        name: "LFM2.5-VL 450M",
        tier: "Fast",
        params: "450M",
        description: "Liquid AI's smallest vision language model (Q4_K_M). Quick and light; reads names well, \
                      collector numbers less reliably (it can mistake a Magic card's power/toughness for one).",
        memory_mb: 600,
        license_name: LFM_LICENSE,
        license_url: LFM_LICENSE_URL,
        homepage: "https://huggingface.co/LiquidAI/LFM2.5-VL-450M-GGUF",
        model: ModelFile {
            name: "LFM2.5-VL-450M-Q4_K_M.gguf",
            url: hf!("LiquidAI/LFM2.5-VL-450M-GGUF" @ "1abed04b6fe71314d8c446a1371c03d7c332266d" / "LFM2.5-VL-450M-Q4_K_M.gguf"),
            size: 229_313_568,
            sha256: "1093f1331319199bbcacdbd7ecc9aa6e5678db6b55073948d0f508be90c8ab68",
        },
        mmproj: ModelFile {
            name: "mmproj-LFM2.5-VL-450m-Q8_0.gguf",
            url: hf!("LiquidAI/LFM2.5-VL-450M-GGUF" @ "1abed04b6fe71314d8c446a1371c03d7c332266d" / "mmproj-LFM2.5-VL-450m-Q8_0.gguf"),
            size: 102_815_168,
            sha256: "ebfc428baa37efad8bae93864f914b2634a09009f91ad59f974fe1a1565d8561",
        },
    },
    ModelSpec {
        id: "lfm2.5-vl-1.6b",
        name: "LFM2.5-VL 1.6B",
        tier: "Balanced",
        params: "1.6B",
        description: "The 1.6B model (Q4_K_M): reads names and numbers clearly better than 450M, about 2–3 times \
                      slower. A good balance for most computers.",
        memory_mb: 1_500,
        license_name: LFM_LICENSE,
        license_url: LFM_LICENSE_URL,
        homepage: "https://huggingface.co/LiquidAI/LFM2.5-VL-1.6B-GGUF",
        model: ModelFile {
            name: "LFM2.5-VL-1.6B-Q4_K_M.gguf",
            url: hf!("LiquidAI/LFM2.5-VL-1.6B-GGUF" @ "36fc16bc95133424921bcc3da009e83b2f23ffb5" / "LFM2.5-VL-1.6B-Q4_K_M.gguf"),
            size: 730_896_256,
            sha256: "aefc3c97c9eb30d9c0dd6af4c38250f5f5106b57c8cf92de7914c7d0a9c94da2",
        },
        mmproj: ModelFile {
            name: "mmproj-LFM2.5-VL-1.6b-Q8_0.gguf",
            url: hf!("LiquidAI/LFM2.5-VL-1.6B-GGUF" @ "36fc16bc95133424921bcc3da009e83b2f23ffb5" / "mmproj-LFM2.5-VL-1.6b-Q8_0.gguf"),
            size: 583_109_888,
            sha256: "2ce89e610c56f3198ece2b86cf61743a08b9307279c89125eb2412ebb908689d",
        },
    },
];

pub fn find(id: &str) -> Option<&'static ModelSpec> {
    MODELS.iter().find(|m| m.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_consistent() {
        let ids: std::collections::HashSet<_> = MODELS.iter().map(|m| m.id).collect();
        assert_eq!(ids.len(), MODELS.len(), "unique ids");
        assert!(find(DEFAULT_MODEL).is_some());
        for m in MODELS {
            for f in m.files() {
                assert!(f.url.ends_with(&format!("/{}", f.name)), "{}", f.url);
                assert!(f.url.starts_with("https://huggingface.co/LiquidAI/"));
                assert_eq!(f.sha256.len(), 64);
            }
            assert!(m.name.ends_with(m.params), "{}", m.name);
            assert!(m.homepage.starts_with("https://huggingface.co/"));
        }
    }
}
