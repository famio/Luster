## 0.3.0

* On iOS and macOS the engine comes prebuilt through Swift Package Manager,
  checked against its checksum, instead of through CocoaPods; it is no longer
  built from source where Rust is installed. Flutter uses Swift Package Manager
  by default; an app that has turned it off cannot use the package on iOS and
  macOS.

## 0.2.0

* `LusterView` takes a `placeholder`, shown while its first badge is struck
  and if that fails.
* The first badge is on screen sooner: it is struck while the view sets up its
  studio, and the mipmaps of its colours are made off the UI thread.
* A line's colour no longer glints along the metal's walls as a badge turns.

## 0.1.0

* First release: `LusterView`, `Luster.mint`, `LusterSnapshot` and GLB export.
