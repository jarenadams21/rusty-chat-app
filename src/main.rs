// Import Rocket
#[macro_use] extern crate rocket;

// Channels pass messages in between different async tasks
use rocket::{State, Shutdown};
use rocket::fs::{relative, FileServer};
use rocket::form::Form;
use rocket::response::stream::{EventStream, Event};
use rocket::serde::{Serialize, Deserialize};
use rocket::tokio::sync::broadcast::{channel, Sender, error::RecvError};
use rocket::tokio::select;

#[derive(Debug, Clone, FromForm, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct Message {
    #[field(validate = len(..30))]
    pub room: String,
    #[field(validate = len(..20))]
    pub username: String,
    pub message: String,
}

// EventStreams are similar to web sockets, but work in one direction
#[get("/events")]
async fn events(queue: &State<Sender<Message>>, mut end: Shutdown) -> EventStream![] {
    // New receiver
    let mut rx = queue.subscribe();

    // Server sent events are produced asynchronously
    // Shutdown resolves after the server is shutdown

    // Infinite series of server events
    EventStream! {
        loop {
            // Select waits on multiple concurrent branches and returns once one completes
            let msg = select! {
                // 1st branch
                // Receiver that waits for new messages, and matches the new message to msg
                msg = rx.recv() => match msg {
                    // Ok variant, all good
                    Ok(msg) => msg,
                    // No more senders, break loop
                    Err(RecvError::Closed) => break,
                    // Receiver lagged too far behind, and was disconnected
                    // Next iteration of loop is then ran
                    Err(RecvError::Lagged(_)) => continue,
                },
                // Waits for shutdown feature to resolve, breaking the infinite loop
                _ = &mut end => break,
            };
            // If no break/error was hit, the select macro returns the message we got from receiver
            // Yield a new server sent event with the new message
            yield Event::json(&msg);
        }
    }
}

#[post("/message", data = "<form>")]
fn post(form: Form<Message>, queue: &State<Sender<Message>>) {
    // A send 'fails' if there are no active subscribers. That's okay
    let _res = queue.send(form.into_inner());
}


#[launch]
fn rocket() -> _ {
    rocket::build()
    // Add state to the server of our rocket instance, 
    // which all rocket access handlers have access to
    // What type of messages? A message struct. 
    // Amount of messages a channel can retain at a given time: 1024
    // The output of the channel function is a tuple containing sender & receiver
    .manage(channel::<Message>(1024).0)
    .mount("/", routes![post, events])
    .mount("/", FileServer::from(relative!("static")))
}