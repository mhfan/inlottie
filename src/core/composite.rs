use super::{render::RenderContext, schema::{MatteMode, VisualLayer}, style::TM2DwO};

/// Backend operations needed only for layer masks and track mattes.
/// Image arguments transfer ownership and must be released even when an operation fails.
pub trait CompositeContext: RenderContext {
    type Offscreen;
    type Image;

    fn begin_offscreen(&mut self) -> Result<Self::Offscreen, Self::Error>;
    fn abort_offscreen(&mut self, target: Self::Offscreen);
    fn end_offscreen(&mut self, target: Self::Offscreen) -> Result<Self::Image, Self::Error>;
    fn apply_masks(&mut self, image: Self::Image, layer: &VisualLayer,
        transform: &TM2DwO<Self::TM2D>, frame: f32) -> Result<Self::Image, Self::Error>;
    fn apply_matte(&mut self, content: Self::Image, matte: Self::Image,
        mode: MatteMode) -> Result<Self::Image, Self::Error>;
    fn present(&mut self, image: Self::Image) -> Result<(), Self::Error>;
    fn discard(&mut self, image: Self::Image);
}

pub(super) struct Compositor<I> { pending: Vec<Pending<I>> }
struct Pending<I> { mode: MatteMode, source: Option<u32>, image: I }

impl<I> Default for Compositor<I> { fn default() -> Self { Self { pending: Vec::new() } } }

impl<I> Compositor<I> {
    /// Renders one layer transactionally. Layer masks are applied before the image
    /// is retained as a track-matte target or presented to its parent target.
    pub(super) fn render<RC: CompositeContext<Image = I>>(&mut self, context: &mut RC,
        layer: &VisualLayer, transform: &TM2DwO<RC::TM2D>, frame: f32,
        draw: impl FnOnce(&mut RC) -> Result<(), RC::Error>) -> Result<(), RC::Error> {
        let pending = self.pending.iter().rposition(|matte| accepts(layer, matte));
        if  pending.is_none() && layer.tt.is_none() && layer.masks.is_empty() {
            return draw(context)
        }

        let target = context.begin_offscreen()?;
        if let Err(error) = draw(context) {
            context.abort_offscreen(target);
            return Err(error)
        }
        let mut image = context.end_offscreen(target)?;
        if !layer.masks.is_empty() {
            image = context.apply_masks(image, layer, transform, frame)?;
        }
        if let Some(index) = pending {
            let matte = self.pending.remove(index);
            image = context.apply_matte(matte.image, image, matte.mode)?;
        }

        if let Some(mode) = layer.tt {
            self.pending.push(Pending { mode, source: layer.tp, image }); Ok(())
        } else { context.present(image) }
    }

    /// A hidden or unsupported matte source contributes transparent coverage and
    /// must consume its target instead of letting it bind to a later layer.
    pub(super) fn skip<RC: CompositeContext<Image = I>>(&mut self,
        context: &mut RC, layer: &VisualLayer) {
        if let Some(index) = self.pending.iter().rposition(|matte| accepts(layer, matte)) {
            context.discard(self.pending.remove(index).image);
        }
    }

    pub(super) fn finish<RC: CompositeContext<Image = I>>(self, context: &mut RC) {
        for matte in self.pending { context.discard(matte.image); }
    }
}

fn accepts<I>(layer: &VisualLayer, matte: &Pending<I>) -> bool {
    !layer.td.is_some_and(|td| !td.as_bool()) && layer.base.ind.is_none_or(|id|
        matte.source.is_none_or(|source| id == source))
}
