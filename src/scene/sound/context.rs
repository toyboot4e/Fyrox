//! Sound context.

use crate::{
    core::{
        pool::{Handle, Pool, Ticket},
        reflect::prelude::*,
        visitor::prelude::*,
    },
    resource::model::Model,
    scene::{
        node::Node,
        sound::{effect::Effect, Sound},
    },
    utils::log::{Log, MessageKind},
};
use fxhash::FxHashSet;
use fyrox_sound::{
    context::DistanceModel,
    effects::{reverb::Reverb, BaseEffect, EffectInput},
    renderer::Renderer,
    source::{SoundSource, SoundSourceBuilder, Status},
};
use std::time::Duration;

/// Sound context.
#[derive(Debug, Visit, Reflect)]
pub struct SoundContext {
    master_gain: f32,
    renderer: Renderer,
    distance_model: DistanceModel,
    paused: bool,
    #[reflect(hidden)]
    pub(crate) effects: Pool<Effect>,
    #[reflect(read_only)]
    // A model resource from which this context was instantiated from.
    pub(crate) resource: Option<Model>,
    #[visit(skip)]
    #[reflect(hidden)]
    pub(crate) native: fyrox_sound::context::SoundContext,
}

impl Default for SoundContext {
    fn default() -> Self {
        Self {
            master_gain: 1.0,
            renderer: Default::default(),
            distance_model: Default::default(),
            paused: false,
            effects: Default::default(),
            resource: None,
            native: fyrox_sound::context::SoundContext::new(),
        }
    }
}

impl SoundContext {
    pub(crate) fn new() -> Self {
        Default::default()
    }

    /// Adds new effect and returns its handle.
    pub fn add_effect(&mut self, effect: Effect) -> Handle<Effect> {
        self.effects.spawn(effect)
    }

    /// Removes specified effect.
    pub fn remove_effect(&mut self, effect: Handle<Effect>) -> Effect {
        self.effects.free(effect)
    }

    /// Borrows an effect.
    pub fn effect(&self, handle: Handle<Effect>) -> &Effect {
        &self.effects[handle]
    }

    /// Borrows an effect as mutable.
    pub fn effect_mut(&mut self, handle: Handle<Effect>) -> &mut Effect {
        &mut self.effects[handle]
    }

    /// Puts effect back using its ticket.
    pub fn put_effect_back(&mut self, ticket: Ticket<Effect>, effect: Effect) -> Handle<Effect> {
        self.effects.put_back(ticket, effect)
    }

    /// Extracts effect from the context with a promise that it'll be returned back.
    pub fn take_reserve_effect(&mut self, handle: Handle<Effect>) -> (Ticket<Effect>, Effect) {
        self.effects.take_reserve(handle)
    }

    /// Makes effect's entry vacant again.
    pub fn forget_effect_ticket(&mut self, ticket: Ticket<Effect>) {
        self.effects.forget_ticket(ticket)
    }

    /// Borrows an effect.
    pub fn try_get_effect(&self, handle: Handle<Effect>) -> Option<&Effect> {
        self.effects.try_borrow(handle)
    }

    /// Borrows an effect as mutable.
    pub fn try_get_effect_mut(&mut self, handle: Handle<Effect>) -> Option<&mut Effect> {
        self.effects.try_borrow_mut(handle)
    }

    /// Returns an iterator over all effects.
    pub fn effects(&self) -> impl Iterator<Item = (Handle<Effect>, &Effect)> {
        self.effects.pair_iter()
    }

    /// Returns total amount of effects.
    pub fn effects_count(&self) -> u32 {
        self.effects.alive_count()
    }

    /// Pause/unpause the sound context. Paused context won't play any sounds.
    pub fn pause(&mut self, pause: bool) {
        self.paused = pause;
        self.native.state().pause(self.paused);
    }

    /// Returns true if the sound context is paused, false - otherwise.
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Sets new distance model.
    pub fn set_distance_model(&mut self, distance_model: DistanceModel) {
        self.distance_model = distance_model;
        self.native.state().set_distance_model(self.distance_model);
    }

    /// Returns current distance model.
    pub fn distance_model(&self) -> DistanceModel {
        self.distance_model
    }

    /// Normalizes given frequency using context's sampling rate. Normalized frequency then can be used
    /// to create filters.
    pub fn normalize_frequency(&self, f: f32) -> f32 {
        self.native.state().normalize_frequency(f)
    }

    /// Returns amount of time context spent on rendering all sound sources.
    pub fn full_render_duration(&self) -> Duration {
        self.native.state().full_render_duration()
    }

    /// Returns current renderer.
    pub fn renderer(&self) -> Renderer {
        self.renderer.clone()
    }

    /// Sets new renderer.
    pub fn set_renderer(&mut self, renderer: Renderer) -> Renderer {
        self.native.state().set_renderer(renderer)
    }

    /// Sets new master gain. Master gain is used to control total sound volume that will be passed to output
    /// device.
    pub fn set_master_gain(&mut self, gain: f32) {
        self.master_gain = gain;
        self.native.state().set_master_gain(self.master_gain)
    }

    /// Returns master gain.
    pub fn master_gain(&self) -> f32 {
        self.master_gain
    }

    /// Destroys all backing sound entities.
    pub fn destroy_sound_sources(&mut self) {
        self.native.state().sources_mut().clear();
    }

    pub(crate) fn update(&mut self) {
        let mut state = self.native.state();

        for effect in self.effects.iter() {
            if effect.native.get().is_some() {
                let native_effect = state.effect_mut(effect.native.get());
                if let (
                    fyrox_sound::effects::Effect::Reverb(native_reverb),
                    Effect::Reverb(reverb),
                ) = (native_effect, effect)
                {
                    reverb.decay_time.try_sync_model(|v| {
                        native_reverb.set_decay_time(Duration::from_secs_f32(v))
                    });
                    reverb.gain.try_sync_model(|v| native_reverb.set_gain(v));
                    reverb.wet.try_sync_model(|v| native_reverb.set_wet(v));
                    reverb.dry.try_sync_model(|v| native_reverb.set_dry(v));
                    reverb.fc.try_sync_model(|v| native_reverb.set_fc(v));
                }
            } else {
                match effect {
                    Effect::Reverb(reverb) => {
                        let mut native_reverb = Reverb::new(BaseEffect::default());
                        native_reverb.set_gain(reverb.gain());
                        native_reverb.set_fc(reverb.fc());
                        native_reverb.set_decay_time(Duration::from_secs_f32(reverb.decay_time()));
                        native_reverb.set_dry(reverb.dry());
                        native_reverb.set_wet(reverb.wet());
                        let native =
                            state.add_effect(fyrox_sound::effects::Effect::Reverb(native_reverb));
                        reverb.native.set(native);
                    }
                }
            }
        }
    }

    pub(crate) fn remove_sound(&mut self, sound: Handle<SoundSource>, name: &str) {
        let mut state = self.native.state();
        if state.is_valid_handle(sound) {
            state.remove_source(sound);

            Log::info(format!(
                "Native sound source was removed for node: {}",
                name
            ));
        }
    }

    pub(crate) fn set_sound_position(&mut self, sound: &Sound) {
        if let Some(source) = self.native.state().try_get_source_mut(sound.native.get()) {
            source.set_position(sound.global_position());
        }
    }

    pub(crate) fn sync_with_sound(&self, sound: &mut Sound) {
        if let Some(source) = self.native.state().try_get_source_mut(sound.native.get()) {
            // Sync back.
            sound.status.set_value_silent(source.status());
            sound.playback_time.set_value_silent(source.playback_time());
        }
    }

    pub(crate) fn sync_to_sound(
        &mut self,
        sound_handle: Handle<Node>,
        sound: &Sound,
        node_overrides: Option<&FxHashSet<Handle<Node>>>,
    ) {
        if !sound.is_globally_enabled()
            || !node_overrides.map_or(true, |f| f.contains(&sound_handle))
        {
            self.remove_sound(sound.native.get(), &sound.name);
            sound.native.set(Default::default());
            return;
        }

        if sound.native.get().is_some() {
            let mut state = self.native.state();
            let source = state.source_mut(sound.native.get());
            sound.buffer.try_sync_model(|v| {
                Log::verify(source.set_buffer(v));
            });
            sound.max_distance.try_sync_model(|v| {
                source.set_max_distance(v);
            });
            sound.rolloff_factor.try_sync_model(|v| {
                source.set_rolloff_factor(v);
            });
            sound.radius.try_sync_model(|v| {
                source.set_radius(v);
            });
            sound.playback_time.try_sync_model(|v| {
                source.set_playback_time(v);
            });
            sound.pitch.try_sync_model(|v| {
                source.set_pitch(v);
            });
            sound.looping.try_sync_model(|v| {
                source.set_looping(v);
            });
            sound.panning.try_sync_model(|v| {
                source.set_panning(v);
            });
            sound.gain.try_sync_model(|v| {
                source.set_gain(v);
            });
            sound
                .spatial_blend
                .try_sync_model(|v| source.set_spatial_blend(v));
            sound.status.try_sync_model(|v| match v {
                Status::Stopped => {
                    Log::verify(source.stop());
                }
                Status::Playing => {
                    source.play();
                }
                Status::Paused => {
                    source.pause();
                }
            });

            // Make sure to not deadlock.
            drop(state);

            sound.effect_name.try_sync_model(|effect_name| {
                if let Some(effect) = self.effects.iter().find(|e| e.name() == effect_name) {
                    let mut state = self.native.state();
                    let native_effect = state.effect_mut(effect.native.get());
                    if let Some(prev) = native_effect
                        .inputs_ref()
                        .iter()
                        .position(|input| input.source() == sound.native.get())
                    {
                        native_effect.remove_input(prev);
                    }
                    native_effect.add_input(EffectInput::direct(sound.native.get()));
                }
            });
        } else {
            match SoundSourceBuilder::new()
                .with_gain(sound.gain())
                .with_opt_buffer(sound.buffer())
                .with_looping(sound.is_looping())
                .with_panning(sound.panning())
                .with_pitch(sound.pitch())
                .with_status(sound.status())
                .with_playback_time(sound.playback_time())
                .with_position(sound.global_position())
                .with_radius(sound.radius())
                .with_max_distance(sound.max_distance())
                .with_rolloff_factor(sound.rolloff_factor())
                .build()
            {
                Ok(source) => {
                    sound.native.set(self.native.state().add_source(source));

                    if let Some(effect) = self
                        .effects
                        .iter()
                        .find(|e| e.name() == sound.effect_name())
                    {
                        let mut state = self.native.state();
                        let native_effect = state.effect_mut(effect.native.get());
                        native_effect.add_input(EffectInput::direct(sound.native.get()));
                    }

                    Log::writeln(
                        MessageKind::Information,
                        format!("Native sound source was created for node: {}", sound.name()),
                    );
                }
                Err(err) => {
                    Log::writeln(
                        MessageKind::Error,
                        format!(
                            "Unable to create native sound source for node: {}. Reason: {:?}",
                            sound.name(),
                            err
                        ),
                    );
                }
            }
        }
    }
}
