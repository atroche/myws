use headless_chrome::{cdtp::page::ScreenshotFormat, Browser, LaunchOptionsBuilder, Tab};
use log::*;
use std::sync::Arc;

mod logging;
mod server;

/// Launches a dumb server that unconditionally serves the given data as a
/// successful html response; launches a new browser and navigates to the
/// server.
///
/// Users must hold on to the server, which stops when dropped.
fn dumb_server(data: &'static str) -> (server::Server, Browser, Arc<Tab>) {
    let server = server::Server::with_dumb_html(data);
    let (browser, tab) = dumb_client(&server);
    (server, browser, tab)
}

fn dumb_client(server: &server::Server) -> (Browser, Arc<Tab>) {
    let browser = Browser::new(LaunchOptionsBuilder::default().build().unwrap()).unwrap();
    let tab = browser.wait_for_initial_tab().unwrap();
    tab.navigate_to(&format!("http://127.0.0.1:{}", server.port()))
        .unwrap();
    (browser, tab)
}

#[test]
fn simple() -> Result<(), failure::Error> {
    logging::enable_logging();
    let (_server, _browser, tab) = dumb_server(include_str!("simple.html"));
    tab.wait_for_element("div#foobar")?;
    Ok(())
}

#[test]
fn actions_on_tab_wont_hang_after_browser_drops() -> Result<(), failure::Error> {
    logging::enable_logging();
    for _ in 0..10 {
        trace!("starting browser, server and tab");
        let (_, browser, tab) = dumb_server(include_str!("simple.html"));
        trace!("dropping browser");
        drop(browser);
        trace!("finding element");
        assert_eq!(true, tab.find_element("div#foobar").is_err());
    }
    Ok(())
}

#[test]
fn form_interaction() -> Result<(), failure::Error> {
    logging::enable_logging();
    let (_server, _browser, tab) = dumb_server(include_str!("form.html"));
    tab.wait_for_element("input#target")?
        .type_into("mothership")?;
    tab.wait_for_element("button")?.click()?;
    let d = tab.wait_for_element("div#protocol")?.get_description()?;
    assert!(d
        .find(|n| n.node_value == "Missiles launched against mothership")
        .is_some());
    tab.wait_for_element("input#sneakattack")?.click()?;
    tab.wait_for_element("button")?.click()?;
    let d = tab.wait_for_element("div#protocol")?.get_description()?;
    assert!(d
        .find(|n| n.node_value == "Comrades, have a nice day!")
        .is_some());
    Ok(())
}

#[test]
fn capture_screenshot() -> Result<(), failure::Error> {
    logging::enable_logging();
    let (_, _browser, tab) = dumb_server(include_str!("simple.html"));
    tab.wait_until_navigated()?;

    let png_data = tab.capture_screenshot(ScreenshotFormat::PNG, true)?;
    let decoder = png::Decoder::new(&png_data[..]);
    let (info, mut reader) = decoder.read_info()?;
    let mut buf = vec![0; info.buffer_size()];
    reader.next_frame(&mut buf)?;
    // Check that the top-left pixel has the background color set in simple.html
    assert_eq!(buf[0..4], [0x11, 0x22, 0x33, 0xff][..]);

    let jpg_data = tab.capture_screenshot(ScreenshotFormat::JPEG(Some(100)), true)?;
    let mut decoder = jpeg_decoder::Decoder::new(&jpg_data[..]);
    let buf = decoder.decode().unwrap();
    // Check that the total compression error is small-ish compared to the expected
    // pixel color
    let err = buf[0..3]
        .iter()
        .zip(&[0x11, 0x22, 0x33])
        .map(|(b, e)| (i32::from(*b) - e).pow(2) as u32)
        .sum::<u32>();
    assert!(err < 5);
    Ok(())
}

#[test]
fn reload() -> Result<(), failure::Error> {
    logging::enable_logging();
    let mut counter = 0;
    let responder = move |r: tiny_http::Request| {
        let response = tiny_http::Response::new(
            200.into(),
            vec![tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html"[..]).unwrap()],
            std::io::Cursor::new(format!(r#"<div id="counter">{}</div>"#, counter)),
            None,
            None,
        );
        counter += 1;
        r.respond(response)
    };
    let server = server::Server::new(responder);
    let (_browser, tab) = dumb_client(&server);
    assert!(tab
        .wait_for_element("div#counter")?
        .get_description()?
        .find(|n| n.node_value == "0")
        .is_some());
    assert!(tab
        .reload(false, None)?
        .wait_for_element("div#counter")?
        .get_description()?
        .find(|n| n.node_value == "1")
        .is_some());
    // TODO test effect of scriptEvaluateOnLoad
    Ok(())
}
