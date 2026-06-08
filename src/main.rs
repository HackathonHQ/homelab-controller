use homelab_controller::LabController;

fn main() {
    let mut controller = LabController::new();

    controller.add_feature("Open source and self-hosted");
    controller.add_feature("Low-cost hardware support");
    controller.add_feature("Custom tool registry");

    let _ = controller.register_tool(
        "wake-node",
        "Wake a sleeping node with Wake-on-LAN",
        "etherwake <mac-address>",
    );
    let _ = controller.register_tool(
        "health-check",
        "Check service health endpoints",
        "curl -fsS http://<host>:<port>/health",
    );

    println!("Homelab Controller");
    println!("==================");
    println!("Features:");
    for feature in controller.features() {
        println!("- {feature}");
    }

    println!("\nCustom tools:");
    for tool in controller.tools() {
        println!("- {}: {} ({})", tool.name, tool.description, tool.command);
    }
}
