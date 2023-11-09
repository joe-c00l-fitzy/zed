use crate::{
    AnyView, AnyWindowHandle, AppCell, AppContext, BackgroundExecutor, Context, ForegroundExecutor,
    Model, ModelContext, Render, Result, Task, View, ViewContext, VisualContext, WindowContext,
    WindowHandle,
};
use anyhow::{anyhow, Context as _};
use derive_more::{Deref, DerefMut};
use std::{future::Future, rc::Weak};

#[derive(Clone)]
pub struct AsyncAppContext {
    pub(crate) app: Weak<AppCell>,
    pub(crate) background_executor: BackgroundExecutor,
    pub(crate) foreground_executor: ForegroundExecutor,
}

impl Context for AsyncAppContext {
    type Result<T> = Result<T>;

    fn build_model<T: 'static>(
        &mut self,
        build_model: impl FnOnce(&mut ModelContext<'_, T>) -> T,
    ) -> Self::Result<Model<T>>
    where
        T: 'static,
    {
        let app = self
            .app
            .upgrade()
            .ok_or_else(|| anyhow!("app was released"))?;
        let mut app = app.borrow_mut();
        Ok(app.build_model(build_model))
    }

    fn update_model<T: 'static, R>(
        &mut self,
        handle: &Model<T>,
        update: impl FnOnce(&mut T, &mut ModelContext<'_, T>) -> R,
    ) -> Self::Result<R> {
        let app = self
            .app
            .upgrade()
            .ok_or_else(|| anyhow!("app was released"))?;
        let mut app = app.borrow_mut();
        Ok(app.update_model(handle, update))
    }

    fn read_model<T, R>(
        &self,
        handle: &Model<T>,
        callback: impl FnOnce(&T, &AppContext) -> R,
    ) -> Self::Result<R>
    where
        T: 'static,
    {
        let app = self.app.upgrade().context("app was released")?;
        let lock = app.borrow();
        Ok(lock.read_model(handle, callback))
    }

    fn update_window<T, F>(&mut self, window: AnyWindowHandle, f: F) -> Result<T>
    where
        F: FnOnce(AnyView, &mut WindowContext<'_>) -> T,
    {
        let app = self.app.upgrade().context("app was released")?;
        let mut lock = app.borrow_mut();
        lock.update_window(window, f)
    }

    fn read_window<R>(
        &self,
        window: &AnyWindowHandle,
        read: impl FnOnce(AnyView, &AppContext) -> R,
    ) -> Result<R> {
        let app = self.app.upgrade().context("app was released")?;
        let lock = app.borrow();
        lock.read_window(window, read)
    }
}

impl AsyncAppContext {
    pub fn refresh(&mut self) -> Result<()> {
        let app = self
            .app
            .upgrade()
            .ok_or_else(|| anyhow!("app was released"))?;
        let mut lock = app.borrow_mut();
        lock.refresh();
        Ok(())
    }

    pub fn background_executor(&self) -> &BackgroundExecutor {
        &self.background_executor
    }

    pub fn foreground_executor(&self) -> &ForegroundExecutor {
        &self.foreground_executor
    }

    pub fn update<R>(&self, f: impl FnOnce(&mut AppContext) -> R) -> Result<R> {
        let app = self
            .app
            .upgrade()
            .ok_or_else(|| anyhow!("app was released"))?;
        let mut lock = app.borrow_mut();
        Ok(f(&mut *lock))
    }

    pub fn open_window<V>(
        &self,
        options: crate::WindowOptions,
        build_root_view: impl FnOnce(&mut WindowContext) -> View<V>,
    ) -> Result<WindowHandle<V>>
    where
        V: Render,
    {
        let app = self
            .app
            .upgrade()
            .ok_or_else(|| anyhow!("app was released"))?;
        let mut lock = app.borrow_mut();
        Ok(lock.open_window(options, build_root_view))
    }

    pub fn spawn<Fut, R>(&self, f: impl FnOnce(AsyncAppContext) -> Fut) -> Task<R>
    where
        Fut: Future<Output = R> + 'static,
        R: 'static,
    {
        self.foreground_executor.spawn(f(self.clone()))
    }

    pub fn has_global<G: 'static>(&self) -> Result<bool> {
        let app = self
            .app
            .upgrade()
            .ok_or_else(|| anyhow!("app was released"))?;
        let app = app.borrow_mut();
        Ok(app.has_global::<G>())
    }

    pub fn read_global<G: 'static, R>(&self, read: impl FnOnce(&G, &AppContext) -> R) -> Result<R> {
        let app = self
            .app
            .upgrade()
            .ok_or_else(|| anyhow!("app was released"))?;
        let app = app.borrow_mut();
        Ok(read(app.global(), &app))
    }

    pub fn try_read_global<G: 'static, R>(
        &self,
        read: impl FnOnce(&G, &AppContext) -> R,
    ) -> Option<R> {
        let app = self.app.upgrade()?;
        let app = app.borrow_mut();
        Some(read(app.try_global()?, &app))
    }

    pub fn update_global<G: 'static, R>(
        &mut self,
        update: impl FnOnce(&mut G, &mut AppContext) -> R,
    ) -> Result<R> {
        let app = self
            .app
            .upgrade()
            .ok_or_else(|| anyhow!("app was released"))?;
        let mut app = app.borrow_mut();
        Ok(app.update_global(update))
    }
}

#[derive(Clone, Deref, DerefMut)]
pub struct AsyncWindowContext {
    #[deref]
    #[deref_mut]
    app: AsyncAppContext,
    window: AnyWindowHandle,
}

impl AsyncWindowContext {
    pub(crate) fn new(app: AsyncAppContext, window: AnyWindowHandle) -> Self {
        Self { app, window }
    }

    pub fn update<R>(
        &mut self,
        update: impl FnOnce(AnyView, &mut WindowContext) -> R,
    ) -> Result<R> {
        self.app.update_window(self.window, update)
    }

    pub fn on_next_frame(&mut self, f: impl FnOnce(&mut WindowContext) + 'static) {
        self.window.update(self, |_, cx| cx.on_next_frame(f)).ok();
    }

    pub fn read_global<G: 'static, R>(
        &mut self,
        read: impl FnOnce(&G, &WindowContext) -> R,
    ) -> Result<R> {
        self.window.update(self, |_, cx| read(cx.global(), cx))
    }

    pub fn update_global<G, R>(
        &mut self,
        update: impl FnOnce(&mut G, &mut WindowContext) -> R,
    ) -> Result<R>
    where
        G: 'static,
    {
        self.window.update(self, |_, cx| cx.update_global(update))
    }

    pub fn spawn<Fut, R>(&self, f: impl FnOnce(AsyncWindowContext) -> Fut) -> Task<R>
    where
        Fut: Future<Output = R> + 'static,
        R: 'static,
    {
        self.foreground_executor.spawn(f(self.clone()))
    }
}

impl Context for AsyncWindowContext {
    type Result<T> = Result<T>;

    fn build_model<T>(
        &mut self,
        build_model: impl FnOnce(&mut ModelContext<'_, T>) -> T,
    ) -> Result<Model<T>>
    where
        T: 'static,
    {
        self.window
            .update(self, |_, cx| cx.build_model(build_model))
    }

    fn update_model<T: 'static, R>(
        &mut self,
        handle: &Model<T>,
        update: impl FnOnce(&mut T, &mut ModelContext<'_, T>) -> R,
    ) -> Result<R> {
        self.window
            .update(self, |_, cx| cx.update_model(handle, update))
    }

    fn update_window<T, F>(&mut self, window: AnyWindowHandle, update: F) -> Result<T>
    where
        F: FnOnce(AnyView, &mut WindowContext<'_>) -> T,
    {
        self.app.update_window(window, update)
    }

    fn read_model<T, R>(
        &self,
        handle: &Model<T>,
        read: impl FnOnce(&T, &AppContext) -> R,
    ) -> Self::Result<R>
    where
        T: 'static,
    {
        self.app.read_model(handle, read)
    }

    fn read_window<R>(
        &self,
        window: &AnyWindowHandle,
        read: impl FnOnce(AnyView, &AppContext) -> R,
    ) -> Result<R> {
        self.app.read_window(window, read)
    }
}

impl VisualContext for AsyncWindowContext {
    fn build_view<V>(
        &mut self,
        build_view_state: impl FnOnce(&mut ViewContext<'_, V>) -> V,
    ) -> Self::Result<View<V>>
    where
        V: 'static + Render,
    {
        self.window
            .update(self, |_, cx| cx.build_view(build_view_state))
    }

    fn update_view<V: 'static, R>(
        &mut self,
        view: &View<V>,
        update: impl FnOnce(&mut V, &mut ViewContext<'_, V>) -> R,
    ) -> Self::Result<R> {
        self.window
            .update(self, |_, cx| cx.update_view(view, update))
    }

    fn replace_root_view<V>(
        &mut self,
        build_view: impl FnOnce(&mut ViewContext<'_, V>) -> V,
    ) -> Self::Result<View<V>>
    where
        V: Render,
    {
        self.window
            .update(self, |_, cx| cx.replace_root_view(build_view))
    }
}
