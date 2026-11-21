//! End-to-end scenarios of the engine against the fake backend.

use composition_core::{
    BackendError, BackendEvent, BlendMode, BufferId, BufferInfo, BufferState, Capabilities,
    DisplayEvent, Engine, Error, FormatModifier, Frame, Layer, Modifier, OutputCaps, OutputId,
    PixelFormat, PlanError, PlaneCaps, PlaneId, PlaneKind, PresentError, Rect, RejectReasons, Size,
    SubmissionId, Zpos,
};
use composition_fake::{FakeBackend, PlaneState};

const OUT: OutputId = OutputId(0);
const FULL: Rect = Rect::new(0, 0, 1280, 720);

const APP_A: BufferId = BufferId(1);
const APP_B: BufferId = BufferId(2);
const SAFETY: BufferId = BufferId(3);

const VID: PlaneId = PlaneId(31);
const VIDL: PlaneId = PlaneId(41);

fn plane(id: PlaneId, kind: PlaneKind, zpos: i32, formats: &[PixelFormat]) -> PlaneCaps {
    let mut caps = PlaneCaps {
        id,
        kind,
        outputs: 1,
        zpos: Zpos::Fixed(zpos),
        blend_modes: BlendMode::Opaque.mask() | BlendMode::Premultiplied.mask(),
        ..PlaneCaps::default()
    };
    for format in formats {
        caps.formats.push(FormatModifier::linear(*format)).unwrap();
    }
    caps
}

/// Two planes in the style of the application-below-safety-layer scenario:
/// an RGB565 plane underneath an ARGB8888 plane.
fn two_plane_caps(replace_pending: bool) -> Capabilities {
    let mut caps = Capabilities {
        replace_pending,
        ..Capabilities::default()
    };
    caps.outputs
        .push(OutputCaps {
            id: OUT,
            size: Size::new(1280, 720),
            refresh_mhz: 60_000,
        })
        .unwrap();
    caps.planes
        .push(plane(VID, PlaneKind::Primary, 0, &[PixelFormat::RGB565]))
        .unwrap();
    caps.planes
        .push(plane(VIDL, PlaneKind::Overlay, 1, &[PixelFormat::ARGB8888]))
        .unwrap();
    caps
}

fn info(format: PixelFormat) -> BufferInfo {
    BufferInfo {
        size: Size::new(1280, 720),
        format,
        modifier: Modifier::LINEAR,
    }
}

fn engine(replace_pending: bool) -> Engine<FakeBackend> {
    let mut engine = Engine::new(FakeBackend::new(two_plane_caps(replace_pending)));
    for (buffer, format) in [
        (APP_A, PixelFormat::RGB565),
        (APP_B, PixelFormat::RGB565),
        (SAFETY, PixelFormat::ARGB8888),
    ] {
        engine.register_buffer(buffer, info(format)).unwrap();
        engine.finish_write(buffer).unwrap();
    }
    engine
}

fn app(buffer: BufferId) -> Layer {
    Layer::new(buffer, FULL, FULL, 0)
}

fn safety() -> Layer {
    let mut layer = Layer::new(SAFETY, FULL, FULL, 10);
    layer.blend = BlendMode::Premultiplied;
    layer
}

fn drain(engine: &mut Engine<FakeBackend>) -> Vec<DisplayEvent> {
    let mut events = Vec::new();
    while let Some(event) = engine.poll_event().unwrap() {
        events.push(event);
    }
    events
}

fn presented(submission: SubmissionId) -> DisplayEvent {
    DisplayEvent::Presented {
        output: OUT,
        submission,
    }
}

#[test]
fn two_layers_reach_two_planes_and_buffers_rotate_safely() {
    let mut engine = engine(false);

    // Safety layer listed first on purpose: stacking comes from z_order.
    let layers = [safety(), app(APP_A)];
    let first = engine
        .present(&Frame {
            output: OUT,
            layers: &layers,
        })
        .unwrap();

    // Accepted, but no vsync yet: nothing is on screen and nothing reported.
    assert_eq!(engine.backend().scanout(OUT), None);
    assert!(drain(&mut engine).is_empty());
    assert_eq!(engine.buffer_state(APP_A), Some(BufferState::Submitted));

    engine.backend_mut().vsync();
    assert_eq!(drain(&mut engine), [presented(first)]);
    assert_eq!(
        engine.backend().scanout(OUT).unwrap(),
        [
            PlaneState {
                plane: VID,
                buffer: APP_A,
                zpos: 0
            },
            PlaneState {
                plane: VIDL,
                buffer: SAFETY,
                zpos: 1
            },
        ]
    );
    assert_eq!(engine.buffer_state(APP_A), Some(BufferState::Scanning));

    // Rotate the application buffer; the safety buffer stays.
    let layers = [safety(), app(APP_B)];
    let second = engine
        .present(&Frame {
            output: OUT,
            layers: &layers,
        })
        .unwrap();
    // The old buffer is still being scanned out until the latch.
    assert_eq!(
        engine.begin_write(APP_A),
        Err(Error::Present(PresentError::BufferInUse(APP_A)))
    );

    engine.backend_mut().vsync();
    assert_eq!(
        drain(&mut engine),
        [presented(second), DisplayEvent::Released { buffer: APP_A }]
    );
    engine.begin_write(APP_A).unwrap();
    assert_eq!(engine.buffer_state(SAFETY), Some(BufferState::Scanning));
}

#[test]
fn unsupported_combination_is_rejected_with_reasons() {
    let mut engine = engine(false);

    // The application buffer on top of the safety layer: the only RGB565
    // plane is the bottom one.
    let mut top_app = app(APP_A);
    top_app.z_order = 20;
    let layers = [safety(), top_app];
    assert_eq!(
        engine.present(&Frame {
            output: OUT,
            layers: &layers
        }),
        Err(Error::Plan(PlanError::NoPlane {
            layer: 1,
            reasons: RejectReasons::FORMAT | RejectReasons::Z_ORDER
        }))
    );

    // A window on a plane that cannot scale.
    let small = [Layer::new(APP_A, FULL, Rect::new(0, 0, 640, 360), 0)];
    assert_eq!(
        engine.present(&Frame {
            output: OUT,
            layers: &small
        }),
        Err(Error::Plan(PlanError::NoPlane {
            layer: 0,
            reasons: RejectReasons::FORMAT | RejectReasons::SCALING
        }))
    );
    assert_eq!(engine.backend().commit_count(), 0);
}

#[test]
fn unfinished_buffer_is_not_shown() {
    let mut engine = engine(false);
    engine.begin_write(APP_A).unwrap();
    let layers = [app(APP_A)];
    assert_eq!(
        engine.present(&Frame {
            output: OUT,
            layers: &layers
        }),
        Err(Error::Present(PresentError::BufferNotReady(APP_A)))
    );
    assert_eq!(engine.backend().commit_count(), 0);
}

#[test]
fn delayed_vsync_makes_the_output_busy() {
    let mut engine = engine(false);
    let a = [app(APP_A)];
    let b = [app(APP_B)];
    let first = engine
        .present(&Frame {
            output: OUT,
            layers: &a,
        })
        .unwrap();
    assert_eq!(
        engine.present(&Frame {
            output: OUT,
            layers: &b
        }),
        Err(Error::Present(PresentError::Busy(OUT)))
    );
    assert_eq!(engine.buffer_state(APP_B), Some(BufferState::Ready));

    engine.backend_mut().vsync();
    assert_eq!(drain(&mut engine), [presented(first)]);
    engine
        .present(&Frame {
            output: OUT,
            layers: &b,
        })
        .unwrap();
}

#[test]
fn replaced_pending_frame_is_reported_as_dropped() {
    let mut engine = engine(true);
    let a = [app(APP_A)];
    let b = [app(APP_B)];
    let first = engine
        .present(&Frame {
            output: OUT,
            layers: &a,
        })
        .unwrap();
    let second = engine
        .present(&Frame {
            output: OUT,
            layers: &b,
        })
        .unwrap();
    assert_eq!(
        drain(&mut engine),
        [
            DisplayEvent::Dropped {
                output: OUT,
                submission: first
            },
            DisplayEvent::Released { buffer: APP_A }
        ]
    );
    engine.backend_mut().vsync();
    assert_eq!(drain(&mut engine), [presented(second)]);
    assert_eq!(engine.backend().scanout(OUT).unwrap()[0].buffer, APP_B);
}

#[test]
fn failed_commit_changes_nothing() {
    let mut engine = engine(false);
    let a = [app(APP_A)];
    let frame = Frame {
        output: OUT,
        layers: &a,
    };
    engine.backend_mut().fail_next_commits(1);
    assert_eq!(
        engine.present(&frame),
        Err(Error::Backend(BackendError::Platform(-5)))
    );
    assert_eq!(engine.buffer_state(APP_A), Some(BufferState::Ready));
    assert!(drain(&mut engine).is_empty());

    // The failed attempt consumed no submission identifier.
    assert_eq!(engine.present(&frame), Ok(SubmissionId(1)));
}

#[test]
fn backend_check_can_veto_what_the_capabilities_allow() {
    let mut engine = engine(false);
    engine
        .backend_mut()
        .set_check_hook(|_, plan| plan.assignments().len() < 2);
    let both = [app(APP_A), safety()];
    assert_eq!(
        engine.present(&Frame {
            output: OUT,
            layers: &both
        }),
        Err(Error::Plan(PlanError::BackendRejected { candidates: 1 }))
    );
    let one = [app(APP_A)];
    engine
        .present(&Frame {
            output: OUT,
            layers: &one,
        })
        .unwrap();
}

#[test]
fn plan_is_bound_to_its_frame_and_to_the_capabilities() {
    let mut engine = engine(false);
    let a = [app(APP_A)];
    let b = [app(APP_B)];
    let frame = Frame {
        output: OUT,
        layers: &a,
    };
    let plan = engine.validate(&frame).unwrap();
    assert_eq!(
        engine.commit(
            &Frame {
                output: OUT,
                layers: &b
            },
            &plan
        ),
        Err(Error::PlanMismatch)
    );

    engine.backend_mut().set_capabilities(two_plane_caps(false));
    assert_eq!(drain(&mut engine), [DisplayEvent::CapabilitiesChanged]);
    assert_eq!(engine.commit(&frame, &plan), Err(Error::StalePlan));

    let plan = engine.validate(&frame).unwrap();
    engine.commit(&frame, &plan).unwrap();
}

#[test]
fn losing_a_plane_turns_a_valid_frame_into_a_rejection() {
    let mut engine = engine(false);
    let layers = [app(APP_A), safety()];
    let frame = Frame {
        output: OUT,
        layers: &layers,
    };
    engine.validate(&frame).unwrap();

    let mut reduced = two_plane_caps(false);
    reduced.planes.pop();
    engine.backend_mut().set_capabilities(reduced);
    drain(&mut engine);
    assert_eq!(
        engine.validate(&frame),
        Err(Error::Plan(PlanError::NoPlane {
            layer: 1,
            reasons: RejectReasons::IN_USE
        }))
    );
}

#[test]
fn device_loss_drops_pending_work_and_releases_buffers() {
    let mut engine = engine(false);
    let a = [app(APP_A), safety()];
    let b = [app(APP_B), safety()];
    let first = engine
        .present(&Frame {
            output: OUT,
            layers: &a,
        })
        .unwrap();
    engine.backend_mut().vsync();
    assert_eq!(drain(&mut engine), [presented(first)]);
    let second = engine
        .present(&Frame {
            output: OUT,
            layers: &b,
        })
        .unwrap();

    engine.backend_mut().lose_device();
    assert_eq!(
        drain(&mut engine),
        [
            DisplayEvent::Dropped {
                output: OUT,
                submission: second
            },
            DisplayEvent::Released { buffer: APP_B },
            DisplayEvent::Released { buffer: SAFETY },
            DisplayEvent::Released { buffer: APP_A },
            DisplayEvent::DeviceLost,
        ]
    );
    for buffer in [APP_A, APP_B, SAFETY] {
        assert_eq!(engine.buffer_state(buffer), Some(BufferState::Ready));
    }
    assert_eq!(
        engine.present(&Frame {
            output: OUT,
            layers: &a
        }),
        Err(Error::Backend(BackendError::DeviceLost))
    );

    engine.backend_mut().restore(two_plane_caps(false));
    assert_eq!(drain(&mut engine), [DisplayEvent::CapabilitiesChanged]);
    engine
        .present(&Frame {
            output: OUT,
            layers: &a,
        })
        .unwrap();
}

#[test]
fn inconsistent_latch_from_the_backend_is_an_error() {
    let mut engine = engine(false);
    engine.backend_mut().inject_event(BackendEvent::Latched {
        output: OUT,
        submission: SubmissionId(77),
    });
    assert_eq!(
        engine.poll_event(),
        Err(Error::Present(PresentError::UnexpectedLatch {
            output: OUT,
            submission: SubmissionId(77)
        }))
    );
    // The engine keeps working afterwards.
    let a = [app(APP_A)];
    let first = engine
        .present(&Frame {
            output: OUT,
            layers: &a,
        })
        .unwrap();
    engine.backend_mut().vsync();
    assert_eq!(drain(&mut engine), [presented(first)]);
}

#[test]
fn producer_stopping_leaves_the_last_frame_on_screen() {
    let mut engine = engine(false);
    let layers = [app(APP_A), safety()];
    let first = engine
        .present(&Frame {
            output: OUT,
            layers: &layers,
        })
        .unwrap();
    engine.backend_mut().vsync();
    assert_eq!(drain(&mut engine), [presented(first)]);

    // No new frames: further vsyncs change nothing and release nothing.
    for _ in 0..5 {
        engine.backend_mut().vsync();
        assert!(drain(&mut engine).is_empty());
    }
    assert_eq!(engine.backend().scanout(OUT).unwrap().len(), 2);

    // Dropping the application layer keeps the safety layer and frees the
    // application buffer once the new frame is latched.
    let only_safety = [safety()];
    let second = engine
        .present(&Frame {
            output: OUT,
            layers: &only_safety,
        })
        .unwrap();
    engine.backend_mut().vsync();
    assert_eq!(
        drain(&mut engine),
        [presented(second), DisplayEvent::Released { buffer: APP_A }]
    );
    assert_eq!(
        engine.backend().scanout(OUT).unwrap(),
        [PlaneState {
            plane: VIDL,
            buffer: SAFETY,
            zpos: 1
        }]
    );
}

#[test]
fn missing_vsync_is_reported_as_a_latch_timeout() {
    let mut engine = engine(false);
    engine.set_latch_timeout(Some(50));
    let a = [app(APP_A)];
    engine.advance_time(100);
    let first = engine
        .present(&Frame {
            output: OUT,
            layers: &a,
        })
        .unwrap();

    engine.advance_time(149);
    assert!(drain(&mut engine).is_empty());
    engine.advance_time(150);
    engine.advance_time(500);
    assert_eq!(
        drain(&mut engine),
        [DisplayEvent::LatchTimeout {
            output: OUT,
            submission: first
        }]
    );
    // The frame is still pending: its buffer stays protected and the output
    // stays busy until the hardware latches or the device is declared lost.
    assert_eq!(engine.buffer_state(APP_A), Some(BufferState::Submitted));

    engine.backend_mut().vsync();
    assert_eq!(drain(&mut engine), [presented(first)]);
}

#[test]
fn layer_off_the_output_does_not_tie_up_its_buffer() {
    let mut engine = engine(false);
    let mut off_screen = safety();
    off_screen.destination = Rect::new(1280, 0, 1280, 720);
    let layers = [app(APP_A), off_screen];
    let first = engine
        .present(&Frame {
            output: OUT,
            layers: &layers,
        })
        .unwrap();
    engine.backend_mut().vsync();
    assert_eq!(drain(&mut engine), [presented(first)]);
    assert_eq!(engine.backend().scanout(OUT).unwrap().len(), 1);
    assert_eq!(engine.buffer_state(SAFETY), Some(BufferState::Ready));
    engine.begin_write(SAFETY).unwrap();
}

/// Random sequences of operations: the engine must never hand a buffer to
/// the producer while the fake hardware still references it.
#[test]
fn random_operation_sequences_never_expose_a_buffer_in_use() {
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self, bound: u32) -> u32 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((self.0 >> 33) as u32) % bound
        }
    }

    for seed in 0..40u64 {
        let replace = seed % 2 == 0;
        let mut engine = engine(replace);
        let mut rng = Lcg(seed);
        let mut released = 0u32;
        for _ in 0..400 {
            match rng.next(8) {
                0..=2 => {
                    let buffer = if rng.next(2) == 0 { APP_A } else { APP_B };
                    let layers = [app(buffer), safety()];
                    let count = 1 + rng.next(2) as usize;
                    let _ = engine.present(&Frame {
                        output: OUT,
                        layers: &layers[..count],
                    });
                }
                3 | 4 => engine.backend_mut().vsync(),
                5 => {
                    let buffer = if rng.next(2) == 0 { APP_A } else { APP_B };
                    if engine.begin_write(buffer).is_ok() {
                        // The producer got the buffer: the hardware must not
                        // be using it, pending or latched.
                        let backend = engine.backend();
                        let scanned = backend
                            .scanout(OUT)
                            .is_some_and(|p| p.iter().any(|s| s.buffer == buffer));
                        assert!(!scanned, "seed {seed}: {buffer:?} written while on screen");
                        assert_eq!(engine.buffer_state(buffer), Some(BufferState::Writable));
                        engine.finish_write(buffer).unwrap();
                    }
                }
                6 => engine.backend_mut().fail_next_commits(1),
                _ => {
                    if rng.next(10) == 0 {
                        engine.backend_mut().lose_device();
                        engine.backend_mut().restore(two_plane_caps(replace));
                    }
                }
            }
            while let Some(event) = engine.poll_event().unwrap() {
                if let DisplayEvent::Released { buffer } = event {
                    released += 1;
                    assert_eq!(
                        engine.buffer_state(buffer),
                        Some(BufferState::Ready),
                        "seed {seed}: released buffer is not idle"
                    );
                }
            }
            // Bookkeeping and fake hardware agree on what is on screen.
            for buffer in [APP_A, APP_B, SAFETY] {
                let scanned = engine
                    .backend()
                    .scanout(OUT)
                    .is_some_and(|p| p.iter().any(|s| s.buffer == buffer));
                assert_eq!(
                    engine.buffer_state(buffer) == Some(BufferState::Scanning),
                    scanned,
                    "seed {seed}: state of {buffer:?} disagrees with the hardware"
                );
            }
        }
        assert!(released > 0, "seed {seed}: nothing was ever released");
    }
}
