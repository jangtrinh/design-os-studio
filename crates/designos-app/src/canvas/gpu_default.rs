//! Whether the canvas starts on the GPU. A software rasterizer (Mesa llvmpipe in a VM, for
//! example) makes every frame so slow that a click, the keys typed after it and the mouse
//! release land in one frame, so such an adapter starts on the CPU tiles unless the user
//! chose the GPU canvas. The rule is plain functions over plain values so a test needs no
//! device.

use eframe::egui_wgpu::wgpu::DeviceType;

/// Lower-case fragments of the adapter name or driver that identify a software rasterizer
/// even when the backend reports another device type (the GL backend labels softpipe, and
/// a Vulkan device may name itself only by its driver).
const SOFTWARE_NAMES: [&str; 5] = ["llvmpipe", "lavapipe", "softpipe", "swiftshader", "microsoft basic render driver"];

/// Preferences -> "Use GPU rendering" tooltip while the default is off for a software renderer.
pub const SOFTWARE_OFF_REASON: &str = "A software renderer was found, so GPU rendering is off.";

/// Whether an adapter draws on the CPU: `DeviceType::Cpu`, or a known software name.
pub fn is_software(device_type: DeviceType, name: &str, driver: &str) -> bool {
    device_type == DeviceType::Cpu || [name, driver].into_iter().any(|text| contains_software_name(&text.to_ascii_lowercase()))
}

fn contains_software_name(lower: &str) -> bool {
    SOFTWARE_NAMES.iter().any(|known| lower.contains(known))
}

/// Whether the adapter eframe picked is a software renderer.
pub fn adapter_is_software(rs: &eframe::egui_wgpu::RenderState) -> bool {
    let info = rs.adapter.get_info();
    is_software(info.device_type, &info.name, &info.driver)
}

/// How the canvas starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    On,
    /// Off because the user saved "off".
    Off,
    /// Off because the adapter is a software renderer and the user has not chosen.
    SoftwareOff,
}

impl Start {
    pub fn is_on(self) -> bool {
        self == Start::On
    }
}

/// The start state. `saved` is the stored "Use GPU rendering" choice (`None`: never set, which
/// must stay distinct from a saved "off"), `gpu_flag` is `--gpu`, `software` is whether the
/// adapter draws on the CPU. `--gpu` and a saved choice both beat the adapter check.
pub fn decide(saved: Option<bool>, gpu_flag: bool, software: bool) -> Start {
    match (gpu_flag, saved) {
        (true, _) | (false, Some(true)) => Start::On,
        (false, Some(false)) => Start::Off,
        (false, None) if software => Start::SoftwareOff,
        (false, None) => Start::On,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use DeviceType::{Cpu, DiscreteGpu, IntegratedGpu, Other, VirtualGpu};

    /// (backend, device type, name, driver) as the backends report them.
    const SOFTWARE: [(&str, DeviceType, &str, &str); 9] = [
        ("Vulkan llvmpipe", Cpu, "llvmpipe (LLVM 15.0.7, 256 bits)", "llvmpipe"),
        ("Vulkan lavapipe", Cpu, "llvmpipe (LLVM 17.0.6, 256 bits)", "llvmpipe"),
        ("Vulkan lavapipe, type unreported", Other, "llvmpipe (LLVM 17.0.6, 256 bits)", "llvmpipe"),
        ("Vulkan lavapipe, named", Other, "Lavapipe", ""),
        ("GL llvmpipe", Cpu, "llvmpipe (LLVM 15.0.7, 256 bits)", ""),
        ("GL softpipe", Other, "softpipe", ""),
        ("Vulkan SwiftShader", Cpu, "SwiftShader Device (Subzero)", "SwiftShader driver"),
        ("GL SwiftShader, type unreported", IntegratedGpu, "Google SwiftShader", ""),
        ("D3D12 WARP, type unreported", Other, "Microsoft Basic Render Driver", ""),
    ];

    const HARDWARE: [(&str, DeviceType, &str, &str); 8] = [
        ("Metal M4 Max", IntegratedGpu, "Apple M4 Max", ""),
        ("Metal AMD", DiscreteGpu, "AMD Radeon Pro 5500M", ""),
        ("Vulkan NVIDIA", DiscreteGpu, "NVIDIA GeForce RTX 4090", "NVIDIA"),
        ("Vulkan Mesa Intel", IntegratedGpu, "Mesa Intel(R) UHD Graphics 620 (KBL GT2)", "intel-mesa"),
        ("Vulkan Mesa AMD", DiscreteGpu, "AMD Radeon RX 7900 XTX (RADV NAVI31)", "radv"),
        ("D3D12 Intel", IntegratedGpu, "Intel(R) Iris(R) Xe Graphics", ""),
        ("D3D12 NVIDIA", DiscreteGpu, "NVIDIA GeForce RTX 3060", ""),
        ("Vulkan virtio-gpu", VirtualGpu, "Virtio-GPU Venus (NVIDIA GeForce RTX 3060)", "venus"),
    ];

    #[test]
    fn a_cpu_adapter_is_software_whatever_it_is_called() {
        assert!(is_software(Cpu, "Anything", ""));
        assert!(is_software(Cpu, "", ""));
    }

    #[test]
    fn known_software_adapters_are_recognised() {
        for (what, ty, name, driver) in SOFTWARE {
            assert!(is_software(ty, name, driver), "{what} must count as a software renderer");
        }
    }

    #[test]
    fn software_names_match_without_regard_to_case() {
        for name in ["LLVMPIPE", "SoftPipe", "swiftshader", "MICROSOFT BASIC RENDER DRIVER"] {
            assert!(is_software(Other, name, ""), "{name}");
        }
        assert!(is_software(Other, "", "LLVMpipe"), "the driver alone identifies llvmpipe");
    }

    #[test]
    fn real_gpus_are_not_software() {
        for (what, ty, name, driver) in HARDWARE {
            assert!(!is_software(ty, name, driver), "{what} must keep the GPU canvas");
        }
    }

    #[test]
    fn start_follows_the_choice_the_flag_and_the_adapter() {
        use Start::{Off, On, SoftwareOff};
        // (saved, --gpu, software) -> start
        let rows = [
            (None, false, false, On),
            (None, false, true, SoftwareOff),
            (None, true, false, On),
            (None, true, true, On),
            (Some(true), false, false, On),
            (Some(true), false, true, On),
            (Some(true), true, false, On),
            (Some(true), true, true, On),
            (Some(false), false, false, Off),
            (Some(false), false, true, Off),
            (Some(false), true, false, On),
            (Some(false), true, true, On),
        ];
        for (saved, flag, software, want) in rows {
            assert_eq!(decide(saved, flag, software), want, "saved={saved:?} --gpu={flag} software={software}");
        }
    }

    #[test]
    fn a_real_gpu_keeps_the_default_it_had_before_the_adapter_check() {
        for saved in [None, Some(true), Some(false)] {
            for flag in [false, true] {
                assert_eq!(decide(saved, flag, false).is_on(), flag || saved.unwrap_or(true), "saved={saved:?} --gpu={flag}");
                assert_ne!(decide(saved, flag, false), Start::SoftwareOff, "a real GPU is never turned off by the adapter check");
            }
        }
    }

    #[test]
    fn only_a_never_set_choice_without_the_flag_is_overridden() {
        assert!(!decide(None, false, true).is_on());
        assert!(decide(Some(true), false, true).is_on(), "an explicit on survives a software renderer");
        assert!(decide(None, true, true).is_on(), "--gpu survives a software renderer");
    }

    #[test]
    fn the_reason_is_one_plain_sentence_under_twelve_words() {
        let words = SOFTWARE_OFF_REASON.split_whitespace().count();
        assert!(words < 12, "{words} words");
        assert!(SOFTWARE_OFF_REASON.is_ascii(), "plain US English");
        assert!(SOFTWARE_OFF_REASON.ends_with('.') && SOFTWARE_OFF_REASON.matches('.').count() == 1, "one sentence with its period");
        assert!(SOFTWARE_OFF_REASON.contains("GPU rendering"), "names the preference it explains");
    }
}
