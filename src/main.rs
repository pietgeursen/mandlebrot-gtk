use ndarray::Array;
use num_complex::Complex;
use std::io::prelude::*;

use std::io::BufWriter;

use gdk::EventMask;
use gdk_pixbuf::{Colorspace, InterpType, Pixbuf};
use gio::prelude::*;
use glib::prelude::*;
use glib::Bytes;
use gtk::prelude::*;
use gtk::{Builder, EventBox, Application, Window, ApplicationWindow, Button, Image, Inhibit};
use std::thread;
use std::sync::{Mutex, Arc};

const NUM_ITERATIONS: i32 = 100;
const LIMIT: f64 = 100.0;

fn next_mandle_num(previous: &Complex<f64>, c: &Complex<f64>) -> Complex<f64> {
    previous.powi(2) + c
}

fn is_in_set(
    previous: &Complex<f64>,
    previous_previous: &Complex<f64>,
    c: &Complex<f64>,
    iter_num: i32,
) -> f64 {
    let next = next_mandle_num(previous, c);

    if next.norm() > LIMIT {
        1.0 / (NUM_ITERATIONS as f64 - iter_num as f64)
    } else if next == *previous_previous {
        0.0
    } else if iter_num == 0 {
        0.0
    } else {
        is_in_set(&next_mandle_num(&next, c), previous, c, iter_num - 1)
    }
}
struct MandlebrotImage{
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

fn new_image(num_points_y: usize, start_x: f64, start_y: f64, width_ratio: f64, scale: f64 )-> MandlebrotImage{

    let num_points_x = (num_points_y as f64 * width_ratio) as usize;
    let end_x = start_x + 3f64 * scale * width_ratio;  
    let end_y = start_y + 3f64 * scale;  

    let linsp = Array::linspace(start_y, end_y, num_points_y);

    let res = linsp.map(|im| {
        let mut linsp2 = Array::linspace(start_x, end_x, num_points_x);
        linsp2.par_map_inplace(|real| {
            let c = Complex::<f64>::new(*real, *im);
            *real = is_in_set(&c, &c, &c, NUM_ITERATIONS)
        });
        linsp2
    });

    let buff = Vec::new();
    let mut w = BufWriter::new(buff);

    res.iter().for_each(|row| {
        row.iter().for_each(|col| {
            let r = (*col * 0xFF as f64) as u8;
            let alpha = if r == 0 { 0 } else { 255 };
            w.write(&[r, 0, 0, alpha]).unwrap();
        });
    });

    let buff = w.into_inner().unwrap();

    MandlebrotImage{
        bytes: buff,
        num_points_x,
        num_points_y
    }
//    let bytes = Bytes::from_owned(buff);
//    let pixbuf = Pixbuf::new_from_bytes(
//        &bytes,
//        Colorspace::Rgb,
//        true,
//        8,
//        num_points_x as i32,
//        num_points_y as i32,
//        num_points_x as i32 * 4,
//    );
//
//    pixbuf
}

fn main() {
    let application =
        Application::new(Some("com.github.gtk-rs.examples.basic"), Default::default())
            .expect("failed to initialize GTK application");

    application.connect_activate(|app| {
        let built = Builder::new_from_file("./glade_layout.glade");
        let win = built.get_object::<Window>("main_window").unwrap();
        let image_event_box = built.get_object::<EventBox>("mandlebrot_event_box").unwrap();
        let image = built.get_object::<Image>("mandlebrot_main_image").unwrap();

        let height_mutex = Arc::new(Mutex::new(0));
        let width_mutex = Arc::new(Mutex::new(0));
        let scale_mutex = Arc::new(Mutex::new(1f64));

        ApplicationWindow::new(app);

        let button = Button::new_with_label("Click me!");
        button.connect_clicked(|_| {
            println!("Clicked!");
        });
//        let pixbuf2 = pixbuf
//            .scale_simple(
//                num_points_in_window as i32 / 3,
//                num_points_in_window as i32 / 3,
//                InterpType::Bilinear,
//            )
//            .unwrap();
        let cloned_image = image.clone();
        let cloned_height = height_mutex.clone();
        let cloned_width = width_mutex.clone();
        let cloned_scale = scale_mutex.clone();
        image_event_box.connect_button_press_event(move |a, e| {

            let height = cloned_height.lock().unwrap();
            let width = cloned_width.lock().unwrap();
            let mut scale = cloned_scale.lock().unwrap();

            *scale *= 0.8;

            do_the_thing(cloned_image.clone(), *height, *width, *scale);

            println!("position!, {:?}", e.get_position());
            println!("coords!, {:?}", e.get_coords());
            println!("root!, {:?}", e.get_root());

            let alloc = a.get_allocation();
            println!("alloc!, {:?}", alloc);
            println!("a!, {:?}", a);
            Inhibit(false)
        });

        let cloned_image = image.clone();
        image_event_box.connect_size_allocate(move |_, allocation| {

            let scale = scale_mutex.lock().unwrap();

            let mut height = height_mutex.lock().unwrap();
            let mut width = width_mutex.lock().unwrap();

            if *height == allocation.height && *width == allocation.width {
                return
            }

            *width = allocation.width;
            *height = allocation.height;

            do_the_thing(cloned_image.clone(), *height, *width, *scale);
        });

        win.show_all();
    });

    application.run(&[]);
}

fn do_the_thing(image: Image, height: i32, width: i32, scale: f64){
    let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

    thread::spawn(move ||{
        let pbuff = new_image(height as usize, -3.5, -1.5, width as f64 / height as f64, scale);
        tx.send(pbuff).unwrap();
    });

    let cloned_image = image.clone();
    rx.attach(None, move |img| {
        let pixbuf = mandlebrot_image_to_pixbuf(img);
        cloned_image.set_from_pixbuf(Some(&pixbuf));
        glib::Continue(false)
    });

}
