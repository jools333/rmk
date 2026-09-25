use rmk::event::{LayerChangeEvent, PointingProcessorEvent, publish_event};
use rmk::input_device::pointing::PointingMode;
use rmk::macros::processor;

#[processor(subscribe = [LayerChangeEvent])]
pub struct PointingProcessorController;

impl PointingProcessorController {
    pub fn new() -> Self {
        Self
    }

    async fn on_layer_change_event(&mut self, event: LayerChangeEvent) {
        match event.0 {
            // Layer 6: MouseSnip - Snipe mode (1:6 scaling divisor, matching ZMK &zip_xy_scaler 1 6)
            6 => {
                publish_event(PointingProcessorEvent {
                    device_id: 255,
                    mode: PointingMode::Sniper(rmk::input_device::pointing::SniperConfig {
                        multiplier: 1,
                        divisor: 6,
                        invert_x: false,
                        invert_y: false,
                    }),
                });
            }
            // Layer 7: MouseScroll - Scroll mode (1:3 scaling divisor, invert Y, matching ZMK)
            7 => {
                publish_event(PointingProcessorEvent {
                    device_id: 255,
                    mode: PointingMode::Scroll(rmk::input_device::pointing::ScrollConfig {
                        multiplier_x: 1,
                        divisor_x: 3,
                        multiplier_y: 1,
                        divisor_y: 3,
                        invert_x: false,
                        invert_y: true,
                    }),
                });
            }
            // Default: normal cursor mode
            _ => {
                publish_event(PointingProcessorEvent {
                    device_id: 255,
                    mode: PointingMode::Cursor(rmk::input_device::pointing::CursorConfig::default()),
                });
            }
        }
    }
}
