use plugitin::plugin;
use plugitin::client::{Plugin, Host};
use serde::{Deserialize, Serialize};

plugin!(CoolPlugin);

struct CoolPlugin {

}

impl Plugin for CoolPlugin {
    type ClientCallInput = ClientInput;
    type ClientCallOutput = ClientOutput;
    type HostCallInput = HostInput;
    type HostCallOutput = HostOutput;

    fn new() -> Self {
        CoolPlugin {

        }
    }

    fn call(&mut self, input: &ClientInput, host: &mut Host<HostInput, HostOutput>) -> ClientOutput {
        match host.call(HostInput::Baz) {
            HostOutput::Qux => {

            }
        }
        ClientOutput::Bar
    }
}

#[derive(Deserialize)]
enum ClientInput {
    Foo
}

#[derive(Serialize)]
enum ClientOutput {
    Bar
}

#[derive(Serialize)]
enum HostInput {
    Baz
}

#[derive(Deserialize)]
enum HostOutput {
    Qux
}