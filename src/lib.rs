#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabTool {
    pub name: String,
    pub description: String,
    pub command: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ControllerError {
    EmptyField(&'static str),
    DuplicateTool(String),
}

#[derive(Debug, Default)]
pub struct LabController {
    features: Vec<String>,
    tools: Vec<LabTool>,
}

impl LabController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_feature(&mut self, feature: impl Into<String>) -> bool {
        let feature = feature.into();
        let trimmed = feature.trim();

        if trimmed.is_empty() || self.features.iter().any(|current| current == trimmed) {
            return false;
        }

        self.features.push(trimmed.to_string());
        true
    }

    pub fn register_tool(
        &mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        command: impl Into<String>,
    ) -> Result<(), ControllerError> {
        let name = name.into().trim().to_string();
        let description = description.into().trim().to_string();
        let command = command.into().trim().to_string();

        if name.is_empty() {
            return Err(ControllerError::EmptyField("name"));
        }

        if description.is_empty() {
            return Err(ControllerError::EmptyField("description"));
        }

        if command.is_empty() {
            return Err(ControllerError::EmptyField("command"));
        }

        if self.tools.iter().any(|tool| tool.name == name) {
            return Err(ControllerError::DuplicateTool(name));
        }

        self.tools.push(LabTool {
            name,
            description,
            command,
        });
        Ok(())
    }

    pub fn features(&self) -> &[String] {
        &self.features
    }

    pub fn tools(&self) -> &[LabTool] {
        &self.tools
    }
}

#[cfg(test)]
mod tests {
    use super::{ControllerError, LabController};

    #[test]
    fn adds_unique_features_only() {
        let mut controller = LabController::new();

        assert!(controller.add_feature("Custom automation"));
        assert!(!controller.add_feature("Custom automation"));
        assert!(!controller.add_feature("   "));
        assert_eq!(controller.features(), &["Custom automation"]);
    }

    #[test]
    fn rejects_invalid_tool_registration() {
        let mut controller = LabController::new();

        assert_eq!(
            controller.register_tool("", "desc", "echo test"),
            Err(ControllerError::EmptyField("name"))
        );
        assert_eq!(
            controller.register_tool("ssh", "desc", ""),
            Err(ControllerError::EmptyField("command"))
        );
    }

    #[test]
    fn stores_custom_tools() {
        let mut controller = LabController::new();

        assert!(
            controller
                .register_tool(
                    "wake-node",
                    "Wake a host through WoL",
                    "etherwake aa:bb:cc:dd:ee:ff"
                )
                .is_ok()
        );

        assert_eq!(controller.tools().len(), 1);
        assert_eq!(controller.tools()[0].name, "wake-node");
        assert_eq!(
            controller.register_tool("wake-node", "duplicate", "echo"),
            Err(ControllerError::DuplicateTool("wake-node".to_string()))
        );
    }
}
