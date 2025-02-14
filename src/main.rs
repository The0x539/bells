use std::io::Read;

use musicxml::datatypes::*;
use musicxml::elements::*;

fn main() {
    let data = if let Some(path) = std::env::args().nth(1) {
        std::fs::read(path).unwrap()
    } else {
        let mut buf = Vec::new();
        std::io::stdin().read_to_end(&mut buf).unwrap();
        buf
    };

    let mxml = musicxml::read_score_data_partwise(data).unwrap();

    let mut state = State::default();
    let mut score = state.score(&mxml);

    score.unify_divisions();
    println!("{score}");
}

pub mod extensions;
use extensions::*;

mod output;

#[derive(Default)]
struct State {
    bpm: Option<(f64, u32)>,
}

impl State {
    fn score(&mut self, score: &ScorePartwise) -> output::Score {
        let mut output = output::Score::default();
        for part in &score.content.part {
            output.parts.push(self.part(part));
        }
        // TODO: get rid of this "state" altogether.
        // problem is that bpm is specified with the measure, not the score,
        // but we care about it score-wide
        output.bpm = self.bpm.unwrap().1;
        output
    }

    fn part(&mut self, part: &Part) -> output::Part {
        let mut output = output::Part::default();
        for element in &part.content {
            match &element {
                PartElement::Measure(m) => output.measures.push(self.measure(m)),
                _ => {}
            };
        }
        output
    }

    fn measure(&mut self, measure: &Measure) -> output::Measure {
        let mut output = output::Measure::default();
        for element in &measure.content {
            match element {
                MeasureElement::Direction(d) => self.direction(d),
                MeasureElement::Note(n) => self.note(n, &mut output),
                MeasureElement::Attributes(a) => {
                    if let Some(divisions) = &a.content.divisions {
                        output.divisions = Some(divisions.content.0);
                    }
                }
                MeasureElement::Backup(..) => break,
                _ => {}
            }
        }
        output
    }

    fn direction(&mut self, direction: &Direction) {
        for ty in &direction.content.direction_type {
            match &ty.content {
                DirectionTypeContents::Metronome(m) => self.metronome(m),
                _ => {}
            }
        }
    }

    fn metronome(&mut self, metronome: &Metronome) {
        match &metronome.content {
            MetronomeContents::BeatBased(beat) => {
                let unit = beat.beat_unit.content;
                let count = match &beat.equals {
                    BeatEquation::BPM(bpm) => bpm.content.parse::<u32>().unwrap(),
                    e => panic!("unhandled beat equation: {e:?}"),
                };
                assert!(self.bpm.is_none());
                self.bpm = Some((unit.as_float(), count));
            }
            m => panic!("unhandled metronome type {m:?}"),
        }
    }

    fn note(&mut self, note: &Note, output: &mut output::Measure) {
        let NoteType::Normal(info) = &note.content.info else {
            println!("eep, non-normal note");
            return;
        };

        let duration = info.duration.content.0;

        let mut output_note = vec![];
        if let AudibleType::Pitch(pitch) = info.audible {
            output_note.push(output::Note {
                step: pitch.content.step.content,
                semitone: pitch.content.alter.map_or(0, |a| a.content.0),
                octave: pitch.content.octave.content.0,
            });
        }

        if info.tie.len() > 0 {
            assert_eq!(info.tie.len(), 1);

            if info.tie[0].attributes.r#type == StartStop::Stop {
                let prev = output.events.last_mut().unwrap();
                assert_eq!(prev.notes, output_note);
                prev.duration += duration;
                return;
            }
        } else if info.chord.is_some() {
            let prev = output.events.last_mut().unwrap();
            assert_eq!(prev.duration, duration);
            prev.notes.extend(output_note);
            return;
        }

        output.events.push(output::Event {
            duration,
            notes: output_note,
        });
    }
}
