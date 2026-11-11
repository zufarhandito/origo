//! Presentation-only roadmap. These cards do not advertise installed converters.
use crate::*;
use gpui::component::{
    ActiveTheme, StyledExt,
    button::{Button, ButtonVariants},
};
use gpui::{AnyElement, IntoElement, div};

struct PlannedTool {
    id: &'static str,
    title: &'static str,
    description: &'static str,
    formats: &'static str,
}
const FROM_PDF: &[PlannedTool] = &[
    PlannedTool {
        id: "pdf-to-images",
        title: "PDF ke gambar",
        description: "Satu gambar untuk setiap halaman PDF.",
        formats: "PNG / JPEG",
    },
    PlannedTool {
        id: "pdf-to-docx",
        title: "PDF ke Word",
        description: "Ubah isi PDF menjadi dokumen Word yang dapat diedit.",
        formats: "DOCX",
    },
    PlannedTool {
        id: "pdf-to-text",
        title: "PDF ke teks",
        description: "Simpan teks dokumen sebagai berkas teks.",
        formats: "TXT",
    },
    PlannedTool {
        id: "pdf-to-xlsx",
        title: "PDF ke Excel",
        description: "Susun tabel PDF menjadi lembar kerja.",
        formats: "XLSX",
    },
    PlannedTool {
        id: "pdf-to-pptx",
        title: "PDF ke PowerPoint",
        description: "Susun isi PDF menjadi presentasi.",
        formats: "PPTX",
    },
];
const TO_PDF: &[PlannedTool] = &[
    PlannedTool {
        id: "images-to-pdf",
        title: "Gambar ke PDF",
        description: "Gabungkan gambar menjadi halaman PDF.",
        formats: "PNG / JPEG ke PDF",
    },
    PlannedTool {
        id: "office-to-pdf",
        title: "Dokumen Office ke PDF",
        description: "Buat PDF dari dokumen, lembar kerja, atau presentasi.",
        formats: "DOCX / XLSX / PPTX ke PDF",
    },
];
fn planned_card(tool: &PlannedTool, cx: &App) -> AnyElement {
    let c = cx.theme().colors;
    div()
        .id(tool.id)
        .accessibility_id(tool.id)
        .role(gpui::Role::Group)
        .aria_label(format!("{} - Akan datang", tool.title))
        .w_full()
        .min_w_0()
        .min_h(px(140.))
        .p_4()
        .rounded_lg()
        .bg(c.background)
        .border_1()
        .border_color(c.border)
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(div().text_sm().font_semibold().child(tool.title))
                .child(
                    div()
                        .text_xs()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(c.accent)
                        .text_color(c.muted_foreground)
                        .child("Akan datang"),
                ),
        )
        .child(
            div()
                .text_sm()
                .text_color(c.muted_foreground)
                .child(tool.description),
        )
        .child(div().flex_1())
        .child(
            div()
                .text_xs()
                .text_color(c.muted_foreground)
                .child(tool.formats),
        )
        .into_any_element()
}
fn planned_group(title: &'static str, tools: &[PlannedTool], columns: u16, cx: &App) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(div().text_lg().font_semibold().child(title))
        .child(
            div()
                .grid()
                .grid_cols(columns)
                .gap_3()
                .children(tools.iter().map(|tool| planned_card(tool, cx))),
        )
        .into_any_element()
}
impl Viewer {
    pub(super) fn conversion_hub(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let c = cx.theme().colors;
        let width = window.viewport_size().width;
        let columns = if width >= px(1280.) {
            4
        } else if width >= px(1000.) {
            3
        } else {
            2
        };
        div().id("conversion-hub").flex_1().min_h_0().w_full().bg(c.muted).overflow_y_scroll()
            .child(div().p_6().flex().flex_col().gap_6()
                .child(div().flex().items_start().justify_between().gap_4()
                    .child(div().flex_1().min_w_0().flex().flex_col().gap_2()
                        .child(div().text_2xl().font_semibold().child("Konversi"))
                        .child(div().text_sm().text_color(c.muted_foreground).child("Fitur berikut sedang direncanakan dan belum dapat dijalankan.")))
                    .child(Button::new("conversion-back").ghost().label("Kembali membaca")
                        .on_click(cx.listener(|this,_,window,cx|this.set_mode(ViewerMode::Read,window,cx)))))
                .child(planned_group("Dari PDF", FROM_PDF, columns, cx))
                .child(planned_group("Ke PDF", TO_PDF, columns, cx))
                .child(div().text_sm().text_color(c.muted_foreground).child("PDF hasil scan memerlukan pengenalan teks (OCR) untuk menghasilkan teks yang dapat diedit. OCR juga belum tersedia.")))
            .into_any_element()
    }
}
