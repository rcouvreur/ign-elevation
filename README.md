Small project to fetch elevation data from the IGN API.

Given a GPS coordinate (latitude and longitude), the program will fetch the elevation at all the points in a 2d grid around the point.

You need Rust to use ```cargo``` and have installed hdf5 dependencies on your system.
To build, go to the directory and run
```
cargo build --release
```
to create the binary in target/release. 
To use the program run for example 
```
ign-elevation 44.90866869 6.2589476894 --image valley.png -r 200 -s 10000
```
This will create an hdf5 file with all elevation data along an image valley.png of the heightmap. The area is centered around the GPS coordinates 44.90866869 6.2589476894, is of size 10km x 10km and given with a resolution of 200 meters.
