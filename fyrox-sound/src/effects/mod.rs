//! Effects module
//!
//! # Overview
//!
//! Provides unified way of creating and using effects.

use crate::{
    context::DistanceModel,
    dsp::filters::Biquad,
    effects::reverb::Reverb,
    listener::Listener,
    source::{SoundSource, Status},
};
use fyrox_core::{
    math,
    pool::{Handle, Pool},
    visitor::{Visit, VisitResult, Visitor},
};
use std::ops::{Deref, DerefMut};

pub mod reverb;

/// Stub effect that does nothing.
#[derive(Default, Debug, Clone, Visit)]
pub struct StubEffect {
    base: BaseEffect,
}

impl EffectRenderTrait for StubEffect {
    fn render(
        &mut self,
        _sources: &Pool<SoundSource>,
        _listener: &Listener,
        _distance_model: DistanceModel,
        _mix_buf: &mut [(f32, f32)],
    ) {
    }
}

impl Deref for StubEffect {
    type Target = BaseEffect;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for StubEffect {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

/// See module docs.
#[derive(Debug, Clone, Visit)]
pub enum Effect {
    /// Stub effect that does nothing.
    Stub(StubEffect),
    /// Reverberation effect. See corresponding module for more info.
    Reverb(Reverb),
}

impl Default for Effect {
    fn default() -> Self {
        Effect::Stub(Default::default())
    }
}

pub(crate) trait EffectRenderTrait {
    fn render(
        &mut self,
        sources: &Pool<SoundSource>,
        listener: &Listener,
        distance_model: DistanceModel,
        mix_buf: &mut [(f32, f32)],
    );
}

/// Base effect for all other kinds of effects. It contains set of inputs (direct
/// or filtered), provides some basic methods to control them.
#[derive(Debug, Clone, Visit)]
pub struct BaseEffect {
    gain: f32,
    inputs: Vec<EffectInput>,
    #[visit(skip)]
    frame_samples: Vec<(f32, f32)>,
}

impl Default for BaseEffect {
    fn default() -> Self {
        Self {
            gain: 1.0,
            inputs: Default::default(),
            frame_samples: Default::default(),
        }
    }
}

impl BaseEffect {
    pub(crate) fn render(
        &mut self,
        sources: &Pool<SoundSource>,
        listener: &Listener,
        distance_model: DistanceModel,
        amount: usize,
    ) {
        // First of all check that inputs are still lead to valid sound sources.
        // We use some sort of weak coupling here - it is ok to leave sound source
        // connected to effect and delete source, such "dangling" inputs will be
        // automatically removed.
        self.inputs
            .retain(|input| sources.is_valid_handle(input.source));

        // Accumulate samples from inputs into accumulation buffer.
        if self.frame_samples.capacity() < amount {
            self.frame_samples = Vec::with_capacity(amount)
        }

        self.frame_samples.clear();
        for _ in 0..amount {
            self.frame_samples.push((0.0, 0.0));
        }

        for input in self.inputs.iter_mut() {
            let source = sources.borrow(input.source);

            if source.status() != Status::Playing {
                continue;
            }

            let distance_gain = source.calculate_distance_gain(listener, distance_model);

            let prev_distance_gain = input.last_distance_gain.unwrap_or(distance_gain);

            input.last_distance_gain = Some(distance_gain);

            let mut k = 0.0;
            let step = 1.0 / amount as f32;

            match input.filter.as_mut() {
                None => {
                    for ((accum_left, accum_right), &(input_left, input_right)) in
                        self.frame_samples.iter_mut().zip(source.frame_samples())
                    {
                        let g = math::lerpf(prev_distance_gain, distance_gain, k);
                        *accum_left += input_left * g;
                        *accum_right += input_right * g;
                        k += step;
                    }
                }
                Some(filter) => {
                    for ((accum_left, accum_right), &(input_left, input_right)) in
                        self.frame_samples.iter_mut().zip(source.frame_samples())
                    {
                        let (filtered_left, filtered_right) = filter.feed(input_left, input_right);
                        let g = math::lerpf(prev_distance_gain, distance_gain, k);
                        *accum_left += filtered_left * g;
                        *accum_right += filtered_right * g;
                        k += step;
                    }
                }
            }
        }
    }

    /// Returns current gain of effect.
    pub fn gain(&self) -> f32 {
        self.gain
    }

    /// Sets effect gain. It should be in (0;1) range, but larger values still fine -
    /// they can be used to achieve "overdrive" effect if needed. Basically this value
    /// defines how "loud" effect will be.
    pub fn set_gain(&mut self, gain: f32) {
        self.gain = gain.max(0.0);
    }

    /// Adds new input to effect.
    pub fn add_input(&mut self, input: EffectInput) {
        self.inputs.push(input)
    }

    /// Returns a reference to all effect inputs.
    pub fn inputs_ref(&self) -> &[EffectInput] {
        &self.inputs
    }

    /// Returns a reference to all effect inputs.
    pub fn inputs_mut(&mut self) -> &mut [EffectInput] {
        &mut self.inputs
    }

    /// Remove an effect input at the given index.
    pub fn remove_input(&mut self, index: usize) -> EffectInput {
        self.inputs.remove(index)
    }

    /// Removes all inputs.
    pub fn clear_inputs(&mut self) {
        self.inputs.clear()
    }
}

/// Input filter is used to transform samples in desired manner, it is based
/// on generic second order biquad filter. See docs for Biquad filter.
#[derive(Default, Debug, Clone, Visit)]
pub struct InputFilter {
    left: Biquad,
    right: Biquad,
}

impl InputFilter {
    /// Creates new instance of input filter using given biquad filter.
    pub fn new(biquad: Biquad) -> Self {
        Self {
            left: biquad.clone(),
            right: biquad,
        }
    }
}

impl InputFilter {
    fn feed(&mut self, left_sample: f32, right_sample: f32) -> (f32, f32) {
        (self.left.feed(left_sample), self.right.feed(right_sample))
    }
}

/// Input is a "reference" to a sound source. Samples of sound source will be
/// either passed directly to effect or will be transformed by filter if one
/// is set.
#[derive(Default, Debug, Clone, Visit)]
pub struct EffectInput {
    /// Handle of source from which effect will take samples each render frame.
    source: Handle<SoundSource>,

    /// Handle of filter that will be used to transform samples. Can be NONE if no
    /// filtering is needed.
    filter: Option<InputFilter>,

    /// Distance gain from last frame, it is used to interpolate distance gain from
    /// frame to frame to prevent clicks in output signal.
    #[visit(skip)]
    last_distance_gain: Option<f32>,
}

impl EffectInput {
    /// Creates new effect input using specified handle of sound source.
    pub fn direct(source: Handle<SoundSource>) -> Self {
        Self {
            source,
            filter: None,
            last_distance_gain: None,
        }
    }

    /// Creates new filtered effect input using specified handles of source and filter.
    ///
    /// Filtered inputs are suitable for emulating occlusion of sound. For example you
    /// can add filter to input and then modify its parameters in runtime: if there is
    /// no direct path from listener to sound source - make it lowpass, otherwise -
    /// allpass.
    pub fn filtered(source: Handle<SoundSource>, filter: InputFilter) -> Self {
        Self {
            source,
            filter: Some(filter),
            last_distance_gain: None,
        }
    }

    /// Returns a handle of the sound source, that is attached to the input.
    pub fn source(&self) -> Handle<SoundSource> {
        self.source
    }

    /// Returns immutable reference to the optional input filter.
    pub fn filter_ref(&mut self) -> Option<&InputFilter> {
        self.filter.as_ref()
    }

    /// Returns mutable reference to the optional input filter.
    pub fn filter_mut(&mut self) -> Option<&mut InputFilter> {
        self.filter.as_mut()
    }
}

macro_rules! static_dispatch {
    ($self:ident, $func:ident, $($args:expr),*) => {
        match $self {
            Effect::Stub(v) => v.$func($($args),*),
            Effect::Reverb(v) => v.$func($($args),*),
        }
    };
}

impl EffectRenderTrait for Effect {
    fn render(
        &mut self,
        sources: &Pool<SoundSource>,
        listener: &Listener,
        distance_model: DistanceModel,
        mix_buf: &mut [(f32, f32)],
    ) {
        static_dispatch!(self, render, sources, listener, distance_model, mix_buf)
    }
}

impl Deref for Effect {
    type Target = BaseEffect;

    fn deref(&self) -> &Self::Target {
        match self {
            Effect::Stub(v) => v,
            Effect::Reverb(v) => v,
        }
    }
}

impl DerefMut for Effect {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Effect::Stub(v) => v,
            Effect::Reverb(v) => v,
        }
    }
}
