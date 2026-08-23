use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use crossbeam_channel::{Receiver, Sender, unbounded};
use image::DynamicImage;
use ratatui::{
    Frame,
    layout::Rect,    widgets::{Block, Borders, Paragraph},
};
use ratatui_image::{
    Resize, StatefulImage,
    picker::Picker,
    protocol::StatefulProtocol,
};
use texp_core::app::App;

pub enum PreviewState {
    Empty,
    Loading,
    Ready(StatefulProtocol),
    Error(String),
}

pub struct PreviewModule {
    picker: Arc<Picker>,
    state: PreviewState,
    last_path: Option<PathBuf>,
    tx_request: Sender<PathBuf>,
    rx_response: Receiver<(PathBuf, Result<DynamicImage, String>)>,
}

impl PreviewModule {
    pub fn new(picker: Arc<Picker>) -> Self {
        let (tx_request, rx_request) = unbounded::<PathBuf>();
        let (tx_response, rx_response) = unbounded::<(PathBuf, Result<DynamicImage, String>)>();

        std::thread::spawn(move || {
            while let Ok(path) = rx_request.recv() {
                match image::open(&path) {
                    Ok(dyn_img) => {
                        let _ = tx_response.send((path, Ok(dyn_img)));
                    }
                    Err(e) => {
                        let _ = tx_response.send((path, Err(e.to_string())));
                    }
                }
            }
        });

        Self {
            picker,
            state: PreviewState::Empty,
            last_path: None,
            tx_request,
            rx_response,
        }
    }

    pub fn sync_path(&mut self, path: Option<&Path>) {
        let target = path.filter(|p| App::is_image(p));
        match target {
            Some(p) if self.last_path.as_deref() == Some(p) => {}
            Some(p) => {
                self.last_path = Some(p.to_path_buf());
                self.state = PreviewState::Loading;
                let _ = self.tx_request.send(p.to_path_buf());
            }
            None => {
                if !matches!(self.state, PreviewState::Empty) || self.last_path.is_some() {
                    self.last_path = None;
                    self.state = PreviewState::Empty;
                }
            }
        }
    }

    pub fn update(&mut self, f: &mut Frame, area: Rect) {
        while let Ok((path, result)) = self.rx_response.try_recv() {
            if self.last_path.as_deref() == Some(path.as_path()) {
                self.state = match result {
                    Ok(image) => PreviewState::Ready(self.picker.new_resize_protocol(image)),
                    Err(e) => PreviewState::Error(e),
                };
            }
        }

        let preview_block = Block::default()
            .borders(Borders::ALL)
            .title(" preview ");
        let inner_area = preview_block.inner(area);
        f.render_widget(preview_block, area);

        match &mut self.state {
            PreviewState::Empty => f.render_widget(Paragraph::new("no preview"), inner_area),
            PreviewState::Loading => f.render_widget(Paragraph::new("loading preview..."), inner_area),
            PreviewState::Error(e) => {
                f.render_widget(Paragraph::new(format!("preview error: {e}")), inner_area)
            }
            PreviewState::Ready(protocol) => {
                let widget = StatefulImage::default().resize(Resize::Fit(None));
                f.render_stateful_widget(widget, inner_area, protocol);
            }
        }
    }
}
