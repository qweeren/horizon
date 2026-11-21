//! An in-game check of `PlayerRef::set_view_center` (horizon-rust/horizon#19219), as a camera mod
//! uses it. Chat `view <chunk x> <chunk z>`: the server streams the chunks around that chunk to
//! you, and the camera looks down on it from above, while you stay where you are. `view off`
//! puts both back on you.
use mod_api::{
    CameraPoseEvent, ChunkPos, ClientHost, Flow, Hooks, ServerChatEvent, ServerHost, Text,
};
use std::sync::Mutex;

/// Each player's centre the server accepted, by UUID: shared by the server and the window of a
/// single-player game, and the window shows only its own player's.
static VIEWS: Mutex<Vec<(u128, (i32, i32))>> = Mutex::new(Vec::new());

#[derive(Default)]
struct Fixture;

/// `view <x> <z>` as Some(Some(chunk)), `view off` as Some(None), anything else as None.
fn parse(text: &str) -> Option<Option<(i32, i32)>> {
    let mut words = text.split_whitespace();
    if words.next()? != "view" {
        return None;
    }
    match (words.next()?, words.next()) {
        ("off", None) => Some(None),
        (x, Some(z)) => Some(Some((x.parse().ok()?, z.parse().ok()?))),
        _ => None,
    }
}

#[mod_api::horizon_mod]
impl Hooks for Fixture {
    fn server_chat(&self, host: &mut dyn ServerHost, event: &mut ServerChatEvent<'_>) -> Flow {
        let Some(view) = parse(event.raw_text) else {
            return Flow::Continue;
        };
        let Some(player) = host.player(event.player) else {
            return Flow::Continue;
        };
        let set = player.set_view_center(host, view.map(|(x, z)| ChunkPos::new(x, z)));
        // A refused call changed nothing on the server, so the camera stays where it was.
        if set {
            let mut views = VIEWS.lock().unwrap();
            views.retain(|&(uuid, _)| uuid != event.player);
            views.extend(view.map(|chunk| (event.player, chunk)));
        }
        event
            .replies
            .push(Text::literal(format!("view centre {view:?}: {set}")));
        Flow::Cancel
    }

    fn camera_pose(&self, host: &dyn ClientHost, event: &mut CameraPoseEvent<'_>) {
        let Some(me) = host.local_player().map(|p| p.uuid()) else {
            return;
        };
        let view = VIEWS
            .lock()
            .unwrap()
            .iter()
            .find(|&&(uuid, _)| uuid == me)
            .map(|&(_, chunk)| chunk);
        if let Some((x, z)) = view {
            // Above the chunk, looking down on it and the ones around it.
            *event.position = [f64::from(x * 16 + 8), 160.0, f64::from(z * 16 - 40)];
            event.pitch = 50.0;
            event.yaw = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn reads_the_chat_commands() {
        assert_eq!(parse("view 40 -12"), Some(Some((40, -12))));
        assert_eq!(parse("view off"), Some(None));
        assert_eq!(parse("view 40"), None);
        assert_eq!(parse("hello"), None);
    }
}
