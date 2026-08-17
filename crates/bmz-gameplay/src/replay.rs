use bmz_core::input::InputEvent;
use bmz_core::replay::ReplayEvent;
use bmz_core::time::TimeUs;

#[derive(Debug, Clone, Default)]
pub struct ReplayRecorder {
    pub events: Vec<ReplayEvent>,
}

impl ReplayRecorder {
    pub fn record(&mut self, input: InputEvent) {
        self.events.push(ReplayEvent {
            lane: input.lane,
            kind: input.kind,
            time: input.time,
            device_kind: input.device_kind,
            scratch_direction: input.scratch_direction,
        });
    }
}

#[derive(Debug, Clone, Default)]
pub struct ReplayPlayer {
    pub events: Vec<ReplayEvent>,
    pub next_index: usize,
}

impl ReplayPlayer {
    pub fn poll_until(&mut self, now: TimeUs) -> Vec<InputEvent> {
        let mut out = Vec::new();
        while let Some(event) = self.events.get(self.next_index).copied() {
            if event.time > now {
                break;
            }
            self.next_index += 1;
            out.push(InputEvent {
                lane: event.lane,
                kind: event.kind,
                time: event.time,
                source: bmz_core::input::InputSource::Replay,
                device_kind: event.device_kind,
                scratch_direction: event.scratch_direction,
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use bmz_core::input::{InputDeviceKind, InputKind, InputSource, ScratchDirection};
    use bmz_core::lane::Lane;

    use super::*;

    #[test]
    fn recorder_and_player_preserve_scratch_direction() {
        let input = InputEvent {
            lane: Lane::Scratch,
            kind: InputKind::Press,
            time: TimeUs(123_456),
            source: InputSource::Human,
            device_kind: InputDeviceKind::Controller,
            scratch_direction: Some(ScratchDirection::Up),
        };
        let mut recorder = ReplayRecorder::default();
        recorder.record(input);

        assert_eq!(recorder.events[0].scratch_direction, Some(ScratchDirection::Up));

        let mut player = ReplayPlayer { events: recorder.events, next_index: 0 };
        let replayed = player.poll_until(TimeUs(123_456));
        assert_eq!(replayed.len(), 1);
        assert_eq!(replayed[0].source, InputSource::Replay);
        assert_eq!(replayed[0].scratch_direction, Some(ScratchDirection::Up));
    }
}
