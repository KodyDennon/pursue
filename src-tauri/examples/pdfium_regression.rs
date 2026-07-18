use anyhow::{Context, Result};
use pdfium_render::prelude::{PdfRenderConfig, Pdfium};
use std::path::PathBuf;

fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let pdf_path = PathBuf::from(
        args.next()
            .context("usage: pdfium_regression <pdf> <pdfium-library> <page>...")?,
    );
    let pdfium_path = PathBuf::from(args.next().context("missing PDFium library path")?);
    let pages = args
        .map(|value| {
            value
                .to_string_lossy()
                .parse::<usize>()
                .context("pages must be positive 1-based integers")
        })
        .collect::<Result<Vec<_>>>()?;
    anyhow::ensure!(!pages.is_empty(), "at least one page is required");

    let bindings = Pdfium::bind_to_library(&pdfium_path)
        .with_context(|| format!("failed to bind {}", pdfium_path.display()))?;
    let pdfium = Pdfium::new(bindings);
    let document = pdfium
        .load_pdf_from_file(&pdf_path, Some(""))
        .or_else(|_| pdfium.load_pdf_from_file(&pdf_path, None))
        .with_context(|| format!("failed to open {}", pdf_path.display()))?;

    for page_number in pages {
        let page_index = page_number.checked_sub(1).context("page 0 is invalid")?;
        let page = document
            .pages()
            .get(page_index as i32)
            .with_context(|| format!("failed to load page {page_number}"))?;
        let bitmap = page
            .render_with_config(
                &PdfRenderConfig::new()
                    .set_target_width(1800)
                    .render_annotations(true)
                    .render_form_data(true),
            )
            .with_context(|| format!("failed to render page {page_number}"))?;
        let image = bitmap
            .as_image()
            .context("failed to convert rendered bitmap")?;
        anyhow::ensure!(
            image.width() > 0 && image.height() > 0,
            "page rendered empty"
        );
        println!("page {page_number}: {}x{}", image.width(), image.height());
    }

    Ok(())
}
