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

/// What a model needs beyond its files: how its answers are prompted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// Liquid AI's LFM2.5-VL.
    Lfm,
    /// Qwen3.5 thinks before answering unless the answer opens with an empty thought, which its chat template adds
    /// when thinking is off; llama.cpp's built-in (ChatML) template leaves it out.
    Qwen35,
}

impl Family {
    /// Text the model's answer starts with, after the formatted prompt.
    pub fn answer_prefix(self) -> &'static str {
        match self {
            Family::Lfm => "",
            Family::Qwen35 => "<think>\n\n</think>\n\n",
        }
    }
}

pub struct ModelSpec {
    pub id: &'static str,
    pub name: &'static str,
    /// "Fast", "Balanced", "Accurate", "Large": where the model sits among the others.
    pub tier: &'static str,
    /// Parameter count as the model card states it, e.g. "450M".
    pub params: &'static str,
    pub description: &'static str,
    /// Memory the loaded model needs, roughly (MB).
    pub memory_mb: u32,
    /// Who made the model, as the license notice names them.
    pub maker: &'static str,
    pub license_name: &'static str,
    pub license_url: &'static str,
    /// The license limits commercial use (LFM: above a revenue threshold).
    pub commercial_limit: bool,
    pub homepage: &'static str,
    pub family: Family,
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
const APACHE_LICENSE: &str = "Apache License 2.0";
const APACHE_LICENSE_URL: &str = "https://www.apache.org/licenses/LICENSE-2.0";

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
        maker: "Liquid AI",
        license_name: LFM_LICENSE,
        license_url: LFM_LICENSE_URL,
        commercial_limit: true,
        homepage: "https://huggingface.co/LiquidAI/LFM2.5-VL-3B-GGUF",
        family: Family::Lfm,
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
        maker: "Liquid AI",
        license_name: LFM_LICENSE,
        license_url: LFM_LICENSE_URL,
        commercial_limit: true,
        homepage: "https://huggingface.co/LiquidAI/LFM2.5-VL-450M-GGUF",
        family: Family::Lfm,
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
        maker: "Liquid AI",
        license_name: LFM_LICENSE,
        license_url: LFM_LICENSE_URL,
        commercial_limit: true,
        homepage: "https://huggingface.co/LiquidAI/LFM2.5-VL-1.6B-GGUF",
        family: Family::Lfm,
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
    ModelSpec {
        id: "qwen3.5-4b",
        name: "Qwen3.5 4B",
        tier: "Accurate",
        params: "4B",
        description: "Alibaba's Qwen3.5 4B (Q4_K_M), under the Apache 2.0 license, which allows commercial use. \
                      Reads card names exactly and leaves a collector number empty rather than guess. About 7 s \
                      per card on an Apple M4: best on a GPU.",
        // Peak RSS on an Apple M4.
        memory_mb: 3_900,
        maker: "Alibaba's Qwen team",
        license_name: APACHE_LICENSE,
        license_url: APACHE_LICENSE_URL,
        commercial_limit: false,
        homepage: "https://huggingface.co/unsloth/Qwen3.5-4B-GGUF",
        family: Family::Qwen35,
        model: ModelFile {
            name: "Qwen3.5-4B-Q4_K_M.gguf",
            url: hf!("unsloth/Qwen3.5-4B-GGUF" @ "e87f176479d0855a907a41277aca2f8ee7a09523" / "Qwen3.5-4B-Q4_K_M.gguf"),
            size: 2_740_937_888,
            sha256: "00fe7986ff5f6b463e62455821146049db6f9313603938a70800d1fb69ef11a4",
        },
        // The projector unquantized: quantizing the vision encoder costs reading accuracy.
        mmproj: ModelFile {
            name: "mmproj-F16.gguf",
            url: hf!("unsloth/Qwen3.5-4B-GGUF" @ "e87f176479d0855a907a41277aca2f8ee7a09523" / "mmproj-F16.gguf"),
            size: 672_423_616,
            sha256: "cd88edcf8d031894960bb0c9c5b9b7e1fea6ebee02b9f7ce925a00d12891f864",
        },
    },
    ModelSpec {
        id: "qwen3.5-9b",
        name: "Qwen3.5 9B",
        tier: "Large",
        params: "9B",
        description: "Alibaba's Qwen3.5 9B (Q4_K_M), the largest model here, under the Apache 2.0 license. Reads \
                      names exactly, like 4B, and is the slowest: about 12 s per card on an Apple M4. Wants a GPU \
                      with 8 GB of memory or more, or a Mac with 16 GB.",
        // Its files plus room to work: what a GPU with its own memory needs. (On Metal the peak RSS, 5.3 GB, leaves
        // out weights the GPU holds.)
        memory_mb: 7_200,
        maker: "Alibaba's Qwen team",
        license_name: APACHE_LICENSE,
        license_url: APACHE_LICENSE_URL,
        commercial_limit: false,
        homepage: "https://huggingface.co/unsloth/Qwen3.5-9B-GGUF",
        family: Family::Qwen35,
        model: ModelFile {
            name: "Qwen3.5-9B-Q4_K_M.gguf",
            url: hf!("unsloth/Qwen3.5-9B-GGUF" @ "3885219b6810b007914f3a7950a8d1b469d598a5" / "Qwen3.5-9B-Q4_K_M.gguf"),
            size: 5_680_522_464,
            sha256: "03b74727a860a56338e042c4420bb3f04b2fec5734175f4cb9fa853daf52b7e8",
        },
        mmproj: ModelFile {
            name: "mmproj-F16.gguf",
            url: hf!("unsloth/Qwen3.5-9B-GGUF" @ "3885219b6810b007914f3a7950a8d1b469d598a5" / "mmproj-F16.gguf"),
            size: 918_166_080,
            sha256: "f70dc3509053962b0d0d3ee8a7eacebf5d60aa560cad78254ae8698516ae029f",
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
                // Each file comes from the model's own repository, at a pinned revision.
                assert!(
                    f.url.starts_with(&format!("{}/resolve/", m.homepage)),
                    "{}",
                    f.url
                );
                assert_eq!(f.sha256.len(), 64);
                assert!(f.size > 0);
            }
            assert!(m.name.ends_with(m.params), "{}", m.name);
            assert!(m.homepage.starts_with("https://huggingface.co/"));
            assert!(m.license_url.starts_with("https://"));
            assert!(
                m.memory_mb as u64 * 1_000_000 > m.size(),
                "{} needs at least its files",
                m.id
            );
        }
    }
}
