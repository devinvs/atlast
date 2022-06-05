use walkdir::WalkDir;
use clap::{App, Arg};
use std::path::Path;
use std::fs::File;
use std::io::BufWriter;
use std::fs::FileType;
use std::io::Write;
use std::io::Cursor;

use serde::Serialize;
use bincode::serialize;

use zip::ZipWriter;
use zip::write::FileOptions;


// x, y, width, height
#[derive(Debug)]
struct Rect {
    x: u32,
    y: u32,
    width: u32,
    height: u32
}

impl Rect {
    fn contains(&self, other: &Rect) -> bool {
        self.contains_point(other.x, other.y) ||
        self.contains_point(other.x+other.width, other.y) ||
        self.contains_point(other.x, other.y+other.height) ||
        self.contains_point(other.x+other.width, other.y+other.height)
    }

    fn contains_point(&self, x: u32, y: u32) -> bool {
        self.x <= x &&
        self.y <= y &&
        self.x + self.width >= x &&
        self.y + self.height >= y
    }
}

#[derive(Serialize, Debug)]
struct AtlasRecord {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    name: String
}


#[derive(Eq, PartialEq)]
struct Image {
    name: String,
    width: u32,
    height: u32,
    data: Vec<u8>
}

impl Image {
    fn area(&self) -> u32 {
        self.width * self.height
    }
}


struct Atlas {
    records: Vec<Rect>,
    images: Vec<Image>,
    width: u32
}

impl Atlas {
    fn new() -> Atlas {
        Atlas {
            records: Vec::new(),
            images: Vec::new(),
            width: 0
        }
    }

    fn add_image(&mut self, path: &Path) {
        let decoder = png::Decoder::new(File::open(path).unwrap());
        let (info, mut reader) = decoder.read_info().unwrap();

        let mut buf = vec![0; info.buffer_size()];
        reader.next_frame(&mut buf).unwrap();

        self.images.push(Image {
            name: path.file_name().unwrap().to_str().unwrap().to_string(),
            width: info.width,
            height: info.height,
            data: buf
        });

        if self.width < info.width {
            self.width = info.width;
        }
    }

    fn pack(&mut self) {
        self.images.sort_unstable_by_key(|img| img.area());
        self.images.reverse();

        for image in self.images.iter() {
            self.records.push(self.next_slot(image.width, image.height));
        }
    }

    fn next_slot(&self, width: u32, height: u32) -> Rect {
        let mut pos = Rect {
            x: 0,
            y: 0,
            width,
            height
        };

        while self.records.iter().any(|rect| rect.contains(&pos)) || pos.x+pos.height > self.width {
            if pos.x == self.width-1 {
                pos.x = 0;
                pos.y += 1;
            } else {
                pos.x += 1;
            }
        }

        pos
    }

    fn write(&mut self, path: &str) {
        if self.images.len() == 0 {
            println!("No images in directory");
            return;
        }

        // Create Buffered Writer for all io ops
        let file = File::create(path).unwrap();
        let w = BufWriter::new(file);

        // Create Zip Writer
        let mut zip = ZipWriter::new(w);

        // Write Texture Atlas
        zip.start_file("atlas.png", FileOptions::default()).unwrap();

        // Width and height of the buffer
        let width = self.width;
        let height = self.records.iter()
            .map(|rect| rect.y+rect.height)
            .max().unwrap();

        // Buffer that the png encoder writes to
        let mut file_buffer = Vec::with_capacity((width*4*height) as usize);

        {
            let w = Cursor::new(&mut file_buffer);

            // Buffer that the encoder reads the png data from
            let mut png_buffer = vec![0; (width * 4 * height) as usize];

            // Png encoder
            let mut encoder = png::Encoder::new(w, width, height);
            encoder.set_color(png::ColorType::RGBA);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();

            // Read all the images into the png buffer with proper placement
            for (image, rect) in self.images.iter().zip(self.records.iter()) {
                for row in 0..image.height {
                    for col in 0..image.width {
                        let img_index = ((row * image.width + col) * 4) as usize;
                        let buf_index = (((row+rect.y) * width + (col+rect.x))*4) as usize;
                        for pix in 0..4 {
                            png_buffer[buf_index+pix] = image.data[img_index+pix];
                        }
                    }
                }
            }

            // Write the png_buffer into its encoded format in the file buffer
            writer.write_image_data(&png_buffer).unwrap();
        }

        // Finally, write the file buffer into the zip file
        zip.write_all(&file_buffer).unwrap();


        // Create zip file for atlas metadata
        zip.start_file("atlas.data", FileOptions::default()).unwrap();
        let atlas_records: Vec<AtlasRecord> = self.records.iter().zip(self.images.iter())
            .map(|(rect, image)| {
                AtlasRecord {
                    x: rect.x as f32 / width as f32,
                    y: rect.y as f32 / height as f32,
                    width: rect.width as f32 / width as f32,
                    height: rect.height as f32 / height as f32,
                    name: image.name.clone()
                }
            })
            .collect();

        zip.write_all(&serialize(&atlas_records).unwrap()).unwrap();

        zip.finish().unwrap();
    }
}


fn main() {
    let matches = App::new("atlast")
        .version("1.0")
        .author("Devin Vander Stelt")
        .about("Create texture atlases that last")
        .arg(Arg::with_name("asset-directory")
             .short("d")
             .value_name("DIR_NAME")
             .takes_value(true)
             .default_value("./"))
        .arg(Arg::with_name("output-file")
             .short("o")
             .takes_value(true)
             .value_name("FILE_NAME")
             .default_value("output.atlas"))
        .get_matches();

    let asset_dir = matches.value_of("asset-directory").unwrap();
    let output_file = matches.value_of("output-file").unwrap();

    let mut atlas = Atlas::new();

    for entry in WalkDir::new(asset_dir) {
        let entry = entry.unwrap();
        if entry.file_type().is_file() {

            let path = entry.path();

            if let Some(extension) = path.extension() {
                match extension.to_str().unwrap() {
                    "png" => {
                        println!("adding {:?}", path);
                        atlas.add_image(path)
                    }
                    _ => {}
                }
            }
        }
    }

    println!("Packing...");
    atlas.pack();

    println!("Writing...");
    atlas.write(output_file);
}
