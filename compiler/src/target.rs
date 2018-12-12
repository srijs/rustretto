use platforms::Platform;

#[derive(Clone, Debug)]
pub struct Target(Platform);

impl Target {
    pub fn new(platform: Platform) -> Self {
        Target(platform)
    }

    pub fn triple(&self) -> &str {
        self.0.target_triple
    }

    pub fn arch(&self) -> &str {
        self.0.target_arch.as_str()
    }

    pub fn os(&self) -> &str {
        self.0.target_os.as_str()
    }

    pub fn os_version_min(&self) -> &str {
        match self.os() {
            "macos" => "10.11",
            _ => unimplemented!(),
        }
    }
}
