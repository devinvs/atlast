# Atlast

A simple texture packer in rust.

## Usage 

To pack all textures inside a directory into an atlas file:

```cargo run -- -d asset_dir -o output.atlas```

## Output

The atlas file is a zip directory containing two files:

- Packed atlas png
- Texture location data

The texture data is serialized with bincode and contains:
- name
- x
- y
- width
- height

For each texture inside the packed image.

## Limitations

Currently only pngs are able to be packed, this could easily be remedied by using a format agnostic crate such as `image`.

Additionally the output format for the texture location data requires the bincode crate to deserialize. It may be better
to use a more common format such as json or yaml.
