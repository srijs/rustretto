#[derive(Clone, Debug)]
pub struct Target(String);

impl Target {
    pub fn new(triple: &str) -> Self {
        Target(triple.to_owned())
    }

    pub fn triple(&self) -> &str {
        &self.0
    }

    pub fn arch(&self) -> &str {
        match &*self.0 {
            "x86_64-apple-darwin" => "x86_64",
            _ => unimplemented!(),
        }
    }

    pub fn os(&self) -> &str {
        match &*self.0 {
            "x86_64-apple-darwin" => "macos",
            _ => unimplemented!(),
        }
    }

    pub fn os_version_min(&self) -> &str {
        match &*self.0 {
            "x86_64-apple-darwin" => "10.11",
            _ => unimplemented!(),
        }
    }
}
