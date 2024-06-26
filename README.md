<a name="readme-top"></a>


## About The Project
A start to learning how to code up a console based roguelike in rust based on a tutorial supplied at https://tomassedovic.github.io/roguelike-tutorial/

Note: Right now this is a single file nightmare. This is because of the way the tutorial is structured. I shall refactor it at some point so it's less insane

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Getting Started
To get up and running follow the steps below

### Prereqs
You need rustc installed to build the code. This is best done by running rustup
Full instructions can be found here: https://www.rust-lang.org/learn/get-started

If you're running this in WSL on windows then you might need an xserver of some sort, dpending on what version of WSL you are running.

You must also install the tcod dependencies as listed here: https://github.com/tomassedovic/tcod-rs#how-to-use-this

Use the relevant instructions for whichever platyform you're building this on.

### Install
1. Clone the repo
```sh
git clone https://github.com/leeds-rust-belt/roguelike-tutorial.git
```
2. Build

Optional as you can just run it but you know ...
```sh
cargo build --release
```
3. Run it

Either navigate to the target/release folder and run:
ToDo - fix packaging to include the stupid font file
```sh
./roguelike-tutorial
```
or via cargo
```sh
cargo run --release
```

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Playing The Game
Instructions, such as they are ...

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Next Steps
* Split the monofile down into sensible modulses to aid readability
* Move away from the deprecated libtcod

<p align="right">(<a href="#readme-top">back to top</a>)</p>
