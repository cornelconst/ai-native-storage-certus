//! Hello World actor component.
//!
//! Provides a greeter actor that receives [`GreetRequest`] messages and prints
//! hello messages.
//!
//! # Quick start
//!
//! ```
//! use example_helloworld::{GreetRequest, GreeterHandler};
//! use component_framework::actor::Actor;
//!
//! let greeter = Actor::simple(GreeterHandler::new());
//! let greeter_handle = greeter.activate().unwrap();
//!
//! greeter_handle.send(GreetRequest { name: "World".into() }).unwrap();
//! greeter_handle.deactivate().unwrap();
//! ```

use component_framework::actor::ActorHandler;
use component_framework::{define_component, define_interface};

// Define an interface for the greeter component.
define_interface! {
    pub IGreeter {
        fn greeting_prefix(&self) -> &str;
    }
}

// Define the component.
define_component! {
    pub HelloWorldComponent {
        version: "0.1.0",
        provides: [IGreeter],
    }
}

impl IGreeter for HelloWorldComponent {
    fn greeting_prefix(&self) -> &str {
        "Hello"
    }
}

/// Message sent to the greeter actor.
#[derive(Debug)]
pub struct GreetRequest {
    pub name: String,
}

/// Actor handler that prints greetings.
pub struct GreeterHandler {
    count: u32,
}

impl GreeterHandler {
    pub fn new() -> Self {
        Self { count: 0 }
    }
}

impl Default for GreeterHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ActorHandler<GreetRequest> for GreeterHandler {
    fn on_start(&mut self) {
        eprintln!("[greeter] Greeter actor started");
    }

    fn handle(&mut self, msg: GreetRequest) {
        self.count += 1;
        println!("  [{}] Hello, {}!", self.count, msg.name);
    }

    fn on_stop(&mut self) {
        eprintln!("[greeter] Greeter stopped after {} greetings", self.count);
    }
}
