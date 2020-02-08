use ndarray::Array;
use num_complex::Complex;
use std::io::prelude::*;

use std::io::BufWriter;

use gdk_pixbuf::{Colorspace, Pixbuf};
use gio::prelude::*;
use glib::Bytes;
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Builder, EventBox, Image, Inhibit, Window};
use std::sync::{Arc, Mutex};
use std::thread;

const NUM_ITERATIONS: i32 = 200;
const LIMIT: f64 = 10.0E6;

fn next_mandle_num(previous: &Complex<f64>, c: &Complex<f64>) -> Complex<f64> {
    previous.powi(2) + c
}

fn next_derivative(
    previous_mandle: &Complex<f64>,
    previous_derivative: &Complex<f64>
) -> Complex<f64>{
    previous_mandle.scale(2.0) * previous_derivative + 1.0
}

fn is_in_set(
    previous: &Complex<f64>,
    previous_previous: &Complex<f64>,
    previous_derivative: &Complex<f64>,
    c: &Complex<f64>,
    iter_num: i32,
) -> f64 {

    let next = next_mandle_num(previous, c);

    if previous.norm() > LIMIT {
        let dist = 2.0 * ((previous.norm() * f64::ln(previous.norm())) / previous_derivative.norm());
        if dist < 0.25 { 100.0 } else { 0.0 }
    } else if next == *previous_previous {
        255.0 
    } else if iter_num == 0 {
        255.0 
    } else {
        let next_deriv = next_derivative(previous, previous_derivative);
        is_in_set(&next_mandle_num(&next, c), previous, &next_deriv, c, iter_num - 1)
    }
}
struct MandlebrotImage {
    bytes: Vec<u8>,
    num_points_x: usize,
    num_points_y: usize,
}

fn mandlebrot_image_to_pixbuf(mb_image: MandlebrotImage) -> Pixbuf {
    let bytes = Bytes::from_owned(mb_image.bytes);
    Pixbuf::new_from_bytes(
        &bytes,
        Colorspace::Rgb,
        true,
        8,
        mb_image.num_points_x as i32,
        mb_image.num_points_y as i32,
        mb_image.num_points_x as i32 * 4,
    )
}

const STARTING_SIZE: f64 = 3f64;

fn new_image(
    num_points_y: usize,
    center_x: f64,
    center_y: f64,
    width_ratio: f64,
    scale: f64,
) -> MandlebrotImage {
    let num_points_x = (num_points_y as f64 * width_ratio) as usize;

    let start_x = center_x - (STARTING_SIZE / 2f64) * scale * width_ratio;
    let end_x = center_x + (STARTING_SIZE / 2f64) * scale * width_ratio;

    let start_y = center_y - (STARTING_SIZE / 2f64) * scale;
    let end_y = center_y + (STARTING_SIZE / 2f64) * scale;

    let linsp = Array::linspace(start_y, end_y, num_points_y);

    let res = linsp.map(|im| {
        let mut linsp2 = Array::linspace(start_x, end_x, num_points_x);
        linsp2.par_map_inplace(|real| {
            let c = Complex::<f64>::new(*real, *im);
            let initial_deriv = Complex::<f64>::new(1.0, 0.0);
            *real = is_in_set(&c, &c, &initial_deriv, &c, NUM_ITERATIONS)
        });
        linsp2
    });

    let buff = Vec::new();
    let mut w = BufWriter::new(buff);

    res.iter().for_each(|row| {
        row.iter().for_each(|col| {
            //let r = (*col * 0xFF as f64) as u8;
            //let alpha = if r == 0 { 0 } else { 255 };
            w.write(&[0, 0, 0, *col as u8]).unwrap();
        });
    });

    let buff = w.into_inner().unwrap();

    MandlebrotImage {
        bytes: buff,
        num_points_x,
        num_points_y,
    }
}

struct Center {
    x: f64,
    y: f64,
}

fn main() {
    let application =
        Application::new(Some("com.github.gtk-rs.examples.basic"), Default::default())
            .expect("failed to initialize GTK application");

    application.connect_activate(|app| {
        let built = Builder::new_from_file("./glade_layout.glade");
        let win = built.get_object::<Window>("main_window").unwrap();
        let image_event_box = built
            .get_object::<EventBox>("mandlebrot_event_box")
            .unwrap();
        let image = built.get_object::<Image>("mandlebrot_main_image").unwrap();

        let height_mutex = Arc::new(Mutex::new(0));
        let width_mutex = Arc::new(Mutex::new(0));
        let scale_mutex = Arc::new(Mutex::new(1f64));
        let center_mutex = Arc::new(Mutex::new(Center { x: 0.0, y: 0.0 }));

        ApplicationWindow::new(app);

        let cloned_image = image.clone();
        let cloned_height = height_mutex.clone();
        let cloned_width = width_mutex.clone();
        let cloned_scale = scale_mutex.clone();
        let cloned_center = center_mutex.clone();

        image_event_box.connect_button_press_event(move |_, e| {
            let height = cloned_height.lock().unwrap();
            let width = cloned_width.lock().unwrap();
            let mut scale = cloned_scale.lock().unwrap();
            let mut center = cloned_center.lock().unwrap();

            let (click_x, click_y) = e.get_position();
            center.x += *scale * (click_x as f64 - (*width as f64 / 2f64)) / *width as f64;
            center.y += *scale * (click_y as f64 - (*height as f64 / 2f64)) / *height as f64;

            if e.get_button() == 1 {
                *scale *= 0.8;
            } else {
                *scale *= 1.2;
            }

            do_the_thing(
                cloned_image.clone(),
                *height,
                *width,
                *scale,
                center.x,
                center.y,
            );

            Inhibit(false)
        });

        let cloned_image = image.clone();
        image_event_box.connect_size_allocate(move |_, allocation| {
            let scale = scale_mutex.lock().unwrap();
            let center = center_mutex.lock().unwrap();

            let mut height = height_mutex.lock().unwrap();
            let mut width = width_mutex.lock().unwrap();

            if *height == allocation.height && *width == allocation.width {
                return;
            }

            *width = allocation.width;
            *height = allocation.height;

            do_the_thing(
                cloned_image.clone(),
                *height,
                *width,
                *scale,
                center.x,
                center.y,
            );
        });

        win.show_all();
    });

    application.run(&[]);
}

fn do_the_thing(image: Image, height: i32, width: i32, scale: f64, center_x: f64, center_y: f64) {
    let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

    thread::spawn(move || {
        let pbuff = new_image(
            height as usize,
            center_x,
            center_y,
            width as f64 / height as f64,
            scale,
        );
        tx.send(pbuff).unwrap();
    });

    let cloned_image = image.clone();
    rx.attach(None, move |img| {
        let pixbuf = mandlebrot_image_to_pixbuf(img);
        cloned_image.set_from_pixbuf(Some(&pixbuf));
        glib::Continue(false)
    });
}
