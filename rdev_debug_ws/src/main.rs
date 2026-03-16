use rdev::{grab, Event, EventType, Button};

fn main() {
    println!("Trying to grab mouse events... (Press Mouse 4 or 5 to see if it's swallowed in browsers)");
    println!("Press Ctrl+C to stop.");
    if let Err(error) = grab(|event| {
        match event.event_type {
            EventType::ButtonPress(Button::Unknown(1)) | EventType::ButtonRelease(Button::Unknown(1)) => {
                println!("Swallowing Mouse 4: {:?}", event.event_type);
                None
            }
            EventType::ButtonPress(Button::Unknown(2)) | EventType::ButtonRelease(Button::Unknown(2)) => {
                println!("Swallowing Mouse 5: {:?}", event.event_type);
                None
            }
            _ => Some(event),
        }
    }) {
        println!("Error: {:?}", error);
    }
}
