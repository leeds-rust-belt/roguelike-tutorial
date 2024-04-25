use std::cmp;
use tcod::colors;
use tcod::colors::*;
use tcod::console::*;
use rand::Rng;
use tcod::map::{FovAlgorithm, Map as FovMap};
use tcod::input::{self, Event, Key, Mouse};

const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;
const PANEL_HEIGHT: i32 = 7;
const PANEL_Y: i32 = SCREEN_HEIGHT - PANEL_HEIGHT;
const BAR_WIDTH: i32 = 20;
const MSG_X: i32 = BAR_WIDTH + 2;
const MSG_WIDTH: i32 = SCREEN_WIDTH - BAR_WIDTH - 2;
const MSG_HEIGHT: usize = PANEL_HEIGHT as usize - 1;

const MAP_WIDTH: i32 = 80;
const MAP_HEIGHT: i32 = 43;
const ROOM_MAX_SIZE: i32 = 10;
const ROOM_MIN_SIZE: i32 = 6;
const MAX_ROOMS: i32 = 30;
const MAX_ROOM_MONSTERS: i32 = 3;
const MAX_ROOM_ITEMS: i32 = 2;

const FOV_ALGO: FovAlgorithm = FovAlgorithm::Basic;
const FOV_LIGHT_WALLS: bool = true;
const TORCH_RADIUS: i32 = 10;

const PLAYER: usize = 0;

#[derive(Debug, Clone, Copy, PartialEq)]
enum PlayerAction {
    TookTurn,
    DidntTakeTurn,
    Exit,
}

#[derive(Clone, Debug, PartialEq)]
enum AI {
    Basic,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Item {
    Heal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum DeathCallback {
    Player,
    Monster,
}

impl DeathCallback {
    fn callback(self, object: &mut Object, game: &mut Game) {
        use DeathCallback::*;
        let cbk: fn(&mut Object, &mut Game) = match self {
            Player => player_death,
            Monster => monster_death,
        };

        cbk(object, game);
    }
}

// colour defs
const DARK_WALL: Color = Color { r: 0, g: 0, b: 100 };
const LIGHT_WALL: Color = Color { r: 130, g: 110, b: 50};
const DARK_GROUND: Color = Color { r: 50, g: 50, b: 150 };
const LIGHT_GROUND: Color = Color { r: 200, g: 180, b: 50 };

const LIMIT_FPS: i32 = 20;

struct Tcod {
    root: Root,
    con: Offscreen,
    panel: Offscreen,
    fov: FovMap,
    key: Key,
    mouse: Mouse,
}

// type definitions
type Map = Vec<Vec<Tile>>;

// the game map
struct Game {
    map: Map,
    messages: Messages,
    inventory: Vec<Object>,
}

struct Messages {
    messages: Vec<(String, Color)>,
}

impl Messages {
    pub fn new() -> Self {
        Self { messages: vec![] }
    }

    pub fn add<T: Into<String>>(&mut self, msg: T, colour: Color) {
        self.messages.push((msg.into(), colour));
    }

    // creating a deque here essentially so let's have an iterator that can go both ways
    // There's a bit of jiggery-pokery going on here. We have to iterate over a vec of different message types potentially.
    // As we don't know exactly what the iterator looks like we just ask for a list of things that implement the double ended iterator trait
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &(String, Color)> {
        self.messages.iter()
    }
}

#[derive(Debug, Clone, Copy)]
struct Rect {
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32
}

impl Rect {
    pub fn new(x1: i32, y1: i32, w: i32, h: i32) -> Self {
        Rect {x1, y1, x2: x1 + w, y2: y1 + h }
    }

    pub fn centre(&self) -> (i32, i32) {
        let centre_x = (self.x1 + self.x2) / 2;
        let centre_y = (self.y1 + self.y2) / 2;
        (centre_x, centre_y)
    }

    pub fn intersects_with(&self, other: &Rect) -> bool {
        (self.x1 <= other.x2) && (self.x2 >= other.x1) && (self.y1 <= other.y2) && (self.y2 >= other.y1)
    }
}

// an in-game object (e.g. player, monster, et al)
#[derive(Debug)]
struct Object {
    x: i32,
    y: i32,
    chr: char,
    colour: Color,
    name: String,
    blocks: bool,
    alive: bool,
    fighter: Option<Fighter>,
    ai: Option<AI>,
    item: Option<Item>,
}

impl Object {
    // Create a new in-game object
    pub fn new(name: &str, x: i32, y: i32, chr: char, colour: Color, blocks: bool) -> Self {
        Object { x, y, chr, colour, name: name.into(), blocks, alive: false, fighter: None, ai: None, item: None }
    }

    pub fn pos(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    pub fn set_pos(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    pub fn distance_to(&self, other: &Object) -> f32 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        ((dx*dx + dy*dy) as f32).sqrt()
    }

    pub fn take_damage(&mut self, damage: i32, game: &mut Game) {
        if let Some(fighter) = self.fighter.as_mut() {
            if damage > 0 {
                fighter.hp -= damage;
            }
        }

        // need to borrow again so can't wrap it in the above
        if let Some(fighter) = self.fighter {
            if fighter.hp <= 0 {
                self.alive = false;
                fighter.on_death.callback(self, game);
            }
        }
    }

    pub fn attack(&mut self, target: &mut Object, game: &mut Game) {
        let damage = self.fighter.map_or(0, |me| me.power) - target.fighter.map_or(0, |opponent| opponent.defence);
        if damage > 0 {
            game.messages.add(format!("{} attacks {} for {} hp", self.name, target.name, damage), WHITE);
            target.take_damage(damage, game);
        } else {
            game.messages.add(format!("{} attacks {} but it has no effect", self.name, target.name), WHITE);
        }
    }

    // draw the Object (this includes setting the colour appropriately etc)
    // Note - the `dyn` keyword dentoes that we're working on a trait rather than a concrete type
    pub fn draw(&self, con: &mut dyn Console) {
        con.set_default_foreground(self.colour);
        con.put_char(self.x, self.y, self.chr, BackgroundFlag::None);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Fighter {
    max_hp: i32,
    hp: i32,
    defence: i32,
    power: i32,
    on_death: DeathCallback,
}

// tile definitions
#[derive(Debug, Clone, Copy)]
struct Tile {
    blocked: bool,
    explored: bool,
    block_sight: bool,
}

impl Tile {
    pub fn empty() -> Self {
        Tile { blocked: false, explored: false, block_sight: false }
    }

    pub fn wall() -> Self {
        Tile { blocked: true, explored: false, block_sight: true }
    }
}

fn get_names_under_mouse(mouse: Mouse, objects: &[Object], fov_map: &FovMap) -> String {
    let (x, y) = (mouse.cx as i32, mouse.cy as i32);
    // This should probably be a filter_map call
    let names = objects.iter().filter(|obj| obj.pos() == (x, y) && fov_map.is_in_fov(obj.x, obj.y)).map(|obj| obj.name.clone()).collect::<Vec<_>>();
    names.join(", ")
}

fn handle_keys(tcod: &mut Tcod, game: &mut Game, objects: &mut Vec<Object>) -> PlayerAction {
    use tcod::input::KeyCode::*;
    use PlayerAction::*;

    // let key = tcod.root.wait_for_keypress(true);
    let player_alive = objects[PLAYER].alive;
    match (tcod.key, tcod.key.text(), player_alive) {
        (Key { code: Enter, alt: true, .. }, _, _) => {
            let fullscreen = tcod.root.is_fullscreen();
            tcod.root.set_fullscreen(!fullscreen);
            DidntTakeTurn
        },
        (Key { code: Up, .. }, _, true) => {
            player_move_or_attack(0, -1, game, objects);
            TookTurn
        },
        (Key { code: Down, .. }, _, true) => {
            player_move_or_attack(0, 1, game, objects);
            TookTurn
        },
        (Key { code: Left, .. }, _, true) => {
            player_move_or_attack(-1, 0, game, objects);
            TookTurn
        },
        (Key { code: Right, .. }, _, true) => {
            player_move_or_attack(1, 0, game, objects);
            TookTurn
        },
        (Key { code: Text, .. }, "g", true) => {
            let item_id = objects.iter().position(|obj| obj.pos() == objects[PLAYER].pos() && obj.item.is_some());
            if let Some(item_id) = item_id {
                pick_item_up(item_id, game, objects);
            };
            TookTurn
        },
        (Key { code: Escape, .. }, _, _) => Exit,
        _ => DidntTakeTurn
    }
}

// map creation functions
fn make_map(objects: &mut Vec<Object>) -> Map {
    let mut map = vec![vec![Tile::wall(); MAP_HEIGHT as usize]; MAP_WIDTH as usize];
    let mut rooms = vec![];

    for _ in 0..MAX_ROOMS {
        // random room size
        let w = rand::thread_rng().gen_range(ROOM_MIN_SIZE..ROOM_MAX_SIZE + 1);
        let h = rand::thread_rng().gen_range(ROOM_MIN_SIZE..ROOM_MAX_SIZE + 1);

        // random room placememnt withing our map bounds
        let x = rand::thread_rng().gen_range(0..MAP_WIDTH - w);
        let y = rand::thread_rng().gen_range(0..MAP_HEIGHT- h);

        let new_room = Rect::new(x, y, w, h);
        let failed = rooms.iter().any(|r| new_room.intersects_with(r));
        if !failed {
            // no intersections so slap the room down
            create_room(new_room, &mut map);
            place_objects(new_room, &map, objects);
            let (sx, sy) = new_room.centre();

            if rooms.is_empty() {
                // first room - let's put @ here! :)
                objects[PLAYER].set_pos(sx, sy);
            } else {
                // Connect this room to the last one
                let (prev_x, prev_y) = rooms[rooms.len() - 1].centre();

                // this could be improved by setting up a cached generator and using that - might do this later ;) 
                if rand::random() {
                    // Horizontal tunnel then vertical
                    create_h_tunnel(prev_x, sx, prev_y, &mut map);
                    create_v_tunnel(prev_y, sy, sx, &mut map);
                } else {
                    // Vertical then horizontal
                    create_v_tunnel(prev_y, sy, prev_x, &mut map);
                    create_h_tunnel(prev_x, sx, sy, &mut map);
                }
            }

            rooms.push(new_room);
        }
    }

    map
}

fn create_room(room: Rect, map: &mut Map) {
    for x in (room.x1 + 1)..room.x2 {
        for y in (room.y1 + 1)..room.y2 {
            map[x as usize][y as usize] = Tile::empty();
        }
    }
}

fn create_h_tunnel(x1: i32, x2: i32, y: i32, map: &mut Map) {
    for x in cmp::min(x1, x2)..(cmp::max(x1, x2) + 1) {
        map[x as usize][y as usize] = Tile::empty();
    }
}

fn create_v_tunnel(y1: i32, y2: i32, x: i32, map: &mut Map) {
    for y in cmp::min(y1, y2)..(cmp::max(y1, y2) + 1) {
        map[x as usize][y as usize] = Tile::empty();
    }
}

fn place_objects(room: Rect, map: &Map, objects: &mut Vec<Object>) {
    let num_monsters = rand::thread_rng().gen_range(0..MAX_ROOM_MONSTERS + 1);
    for _ in 0..num_monsters {
        let x = rand::thread_rng().gen_range(room.x1+1..room.x2);
        let y = rand::thread_rng().gen_range(room.y1+1..room.y2);

        if !is_blocked(x, y, map, objects) {
            let mut monster = if rand::random::<f32>() < 0.8 {
                let mut orc = Object::new("Orc", x, y, 'o', colors::DESATURATED_GREEN, true);
                orc.fighter = Some(Fighter { max_hp: 10, hp: 10, defence: 0, power: 3, on_death: DeathCallback::Monster });
                orc.ai = Some(AI::Basic);
                orc
            } else {
                let mut troll = Object::new("Troll", x, y, 'T', colors::DARKER_GREEN, true);
                troll.fighter = Some(Fighter { max_hp: 16, hp: 16, defence: 1, power: 4, on_death: DeathCallback::Monster });
                troll.ai = Some(AI::Basic);
                troll
            };
    
            monster.alive = true;
            objects.push(monster);
        }
    }

    let num_items = rand::thread_rng().gen_range(0..MAX_ROOM_ITEMS + 1);
    for _ in 0..num_items {
        let x = rand::thread_rng().gen_range(room.x1+1..room.x2);
        let y = rand::thread_rng().gen_range(room.y1+1..room.y2);

        if !is_blocked(x, y, map, objects) {
            let mut pot = Object::new("healing potion", x, y, '!', VIOLET, false);
            pot.item = Some(Item::Heal);
            objects.push(pot);
        }
    }
}

fn is_blocked(x: i32, y: i32, map: &Map, objects: &[Object]) -> bool {
    if map[x as usize][y as usize].blocked {
        return true;
    }

    objects.iter().any(|obj| obj.blocks && obj.pos() == (x, y))
}

fn ai_take_turn(id: usize, tcod: &Tcod, game: &mut Game, objects: &mut [Object]) {
    let (monster_x, monster_y) = objects[id].pos();
    if tcod.fov.is_in_fov(monster_x, monster_y) {
        if objects[id].distance_to(&objects[PLAYER]) >= 2.0 {
            // Let's move closer
            let (px, py) = objects[PLAYER].pos();
            move_towards(id, px, py, &game.map, objects);
        } else if objects[PLAYER].fighter.map_or(false, |m| m.hp > 0) {
            // ATTTACK!!!!!
            let (monster, player) = mut_two(id, PLAYER, objects);
            monster.attack(player, game);
        }
    }
}

fn move_towards(id: usize, target_x: i32, target_y: i32, map: &Map, objects: &mut [Object]) {
    // figure direction vector out
    let dx = target_x - objects[id].x;
    let dy = target_y - objects[id].y;
    let distance = ((dx*dx + dy*dy) as f32).sqrt();

    // normalise vector to unit - mmm type conversions
    let dx = (dx as f32 / distance).round() as i32;
    let dy = (dy as f32 / distance).round() as i32;
    move_by(id, dx, dy, map, objects);
}

// Move this object by the given delta
fn move_by(id: usize, dx: i32, dy: i32, map: &Map, objects: &mut [Object]) {
    let (x, y) = objects[id].pos();
    if !is_blocked(x + dx, y + dy, map, objects) {
        objects[id].set_pos(x + dx, y + dy);
    }
}

fn player_move_or_attack(dx: i32, dy: i32, game: &mut Game, objects: &mut [Object]) {
    let target_pos = (objects[PLAYER].x + dx, objects[PLAYER].y + dy);
    let target_id = objects.iter().position(|obj| obj.fighter.is_some() && obj.pos() == target_pos);

    match target_id {
        Some(target_id) => {
            // Attackable target
            let (player, target) = mut_two(PLAYER, target_id, objects);
            player.attack(target, game);
        },
        None => {
            // Player move
            move_by(PLAYER, dx, dy, &game.map, objects);
        }
    }
}

fn pick_item_up(obj_id: usize, game: &mut Game, objects: &mut Vec<Object>) {
    if game.inventory.len() >= 26 {
        game.messages.add(format!("Your inventory is full. Cannot pick up {}", objects[obj_id].name), RED);
    } else {
        let item = objects.swap_remove(obj_id);
        game.messages.add(format!("You have picked up a {}", item.name), GREEN);
        game.inventory.push(item);
    }
}

fn player_death(player: &mut Object, game: &mut Game) {
    game.messages.add("You died!", RED);
    player.chr = '%';
    player.colour = DARK_RED;
}

fn monster_death(monster: &mut Object, game: &mut Game) {
    game.messages.add(format!("The {} is dead!", monster.name), ORANGE);
    monster.chr = '%';
    monster.colour = DARK_RED;
    monster.blocks = false;
    monster.fighter = None;
    monster.ai = None;
    monster.name = format!("remains of {}", monster.name);
}

// utils

// Extract two mutable entries from the same slice
fn mut_two<T>(first_idx: usize, second_idx: usize, items: &mut [T]) -> (&mut T, &mut T) {
    // ensure we don't try and extract the same thing twice - just panic at this point
    // Note - split_at_mut will panic if any of the indexes are out of bounds
    assert!(first_idx != second_idx);

    // get the higher of the two indexes
    let split_idx = cmp::max(first_idx, second_idx);

    // split into two mutable slices at the split index
    let (slice1, slice2) = items.split_at_mut(split_idx);

    // figure the actual mutable objects we need based on new index positions
    // the first slice will reflect the lower of the two indexes as is
    // the second slice will have the element we split at at position 0
    if first_idx < second_idx {
        (&mut slice1[first_idx], &mut slice2[0])
    } else {
        (&mut slice2[0], &mut slice1[second_idx])
    }
}

// render functions
fn render_all(tcod: &mut Tcod, game: &mut Game, objects: &[Object], fov_recompute: bool) {
    if fov_recompute {
        let player = &objects[0];
        tcod.fov.compute_fov(player.x, player.y, TORCH_RADIUS, FOV_LIGHT_WALLS, FOV_ALGO);
    }

    let mut to_draw: Vec<_> = objects.iter().filter(|obj| tcod.fov.is_in_fov(obj.x, obj.y)).collect();
    to_draw.sort_by(|o1, o2| o1.blocks.cmp(&o2.blocks));
    for obj in &to_draw {
        obj.draw(&mut tcod.con);
    }

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let visible = tcod.fov.is_in_fov(x, y);
            let wall = game.map[x as usize][y as usize].block_sight;
            let colour = match (visible, wall) {
                // outside FOV
                (false, false) => DARK_GROUND,
                (false, true) => DARK_WALL,
                // inside FOV
                (true, false) => LIGHT_GROUND,
                (true, true) => LIGHT_WALL
            };

            // Need to define here as we're borrowing game.map again, but this time as mutable. Previously borrowed as wall.
            let explored = &mut game.map[x as usize][y as usize].explored;
            if visible {
                *explored = true;
            }

            if *explored {
                tcod.con.set_char_background(x, y, colour, BackgroundFlag::Set);
            }
        }
    }

    blit(&tcod.con, (0, 0), (SCREEN_WIDTH, SCREEN_HEIGHT), &mut tcod.root, (0, 0), 1.0, 1.0);

    // Render stats panel
    tcod.panel.set_default_background(BLACK);
    tcod.panel.clear();

    // render in game messages
    // Go backwards from latest to earlier. some message lines may wrap so we won't always know we have th ecorrect number to render
    let mut y = MSG_HEIGHT as i32;
    for &(ref msg, colour) in game.messages.iter().rev() {
        let msg_height = tcod.panel.get_height_rect(MSG_X, y, MSG_WIDTH, 0, msg);
        y -= msg_height;
        if y < 0 {
            break;
        }

        tcod.panel.set_default_foreground(colour);
        tcod.panel.print_rect(MSG_X, y, MSG_WIDTH, 0, msg);

        tcod.panel.set_default_foreground(LIGHT_GREY);
        tcod.panel.print_ex(1, 0, BackgroundFlag::None, TextAlignment::Left, get_names_under_mouse(tcod.mouse, objects, &tcod.fov))
    }
    // get the relevant player stats
    let hp = objects[PLAYER].fighter.map_or(0, |f| f.hp);
    let max_hp = objects[PLAYER].fighter.map_or(0, |f| f.max_hp);
    render_bar(&mut tcod.panel, 1, 1, BAR_WIDTH, "HP", hp, max_hp, LIGHT_RED, DARKER_RED);

    // blit the panel in
    blit(&tcod.panel, (0, 0), (SCREEN_WIDTH, PANEL_HEIGHT), &mut tcod.root, (0, PANEL_Y), 1.0, 1.0);
}

fn render_bar(panel: &mut Offscreen, x: i32, y: i32, total_width: i32, name: &str, value: i32, maximum: i32, bar_colour: Color, back_colour: Color) {
    // render a bar to track a stat (e.g. hp, xp, etc)
    let bar_width = (value as f32 / maximum as f32 * total_width as f32) as i32;

    // render background
    panel.set_default_background(back_colour);
    panel.rect(x, y, total_width, 1, false, BackgroundFlag::Screen);

    // render the actual bar
    panel.set_default_foreground(bar_colour);
    if bar_width > 0 {
        panel.rect(x, y, bar_width, 1, false, BackgroundFlag::Screen);
    }

    // Let's print the actual value as well
    panel.set_default_foreground(WHITE);
    panel.print_ex(x + total_width / 2, y, BackgroundFlag::None, TextAlignment::Center, &format!("{}: {}/{}", name, value, maximum));
}

// Main
fn main() {
    // Initialise and create the root window
    let root = Root::initializer()
        .font("arial10x10.png", FontLayout::Tcod)
        .font_type(FontType::Greyscale)
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("Making a window happen")
        .init();

    let mut tcod = Tcod {
        root, 
        con: Offscreen::new(MAP_WIDTH, MAP_HEIGHT), 
        panel: Offscreen::new(SCREEN_WIDTH, PANEL_HEIGHT),
        fov: FovMap::new(MAP_WIDTH, MAP_HEIGHT),
        key: Default::default(),
        mouse: Default::default(),
    };

    // limit FPS (doesn't really matter for a key input roguelike)
    tcod::system::set_fps(LIMIT_FPS);

    // Game objects
    let mut player = Object::new("Player", 0, 0, '@', WHITE, true);
    player.alive = true;
    player.fighter = Some(Fighter {max_hp: 30, hp: 30, defence: 2, power: 5, on_death: DeathCallback::Player });

    let mut objects = vec![player];
    let mut game = Game { map: make_map(&mut objects), messages: Messages::new(), inventory: vec![] };

    // Set up FOV map
    for x in 0..MAP_WIDTH {
        for y in 0..MAP_HEIGHT {
            tcod.fov.set(x, y, !game.map[x as usize][y as usize].block_sight, !game.map[x as usize][y as usize].blocked);
        }
    }

    let mut prev_pos = (-1, -1);

    game.messages.add("Welcome stranger! Something something foreboding something something death", RED);

    // It's a game; it needs a game loop
    while !tcod.root.window_closed() {
        let fov_recompute = prev_pos != (objects[PLAYER].x, objects[PLAYER].y);

        // This call panics. :(
        // Might be time to move away from tcod
        match input::check_for_event(input::MOUSE | input::KEY_PRESS) {
            Some((_, Event::Mouse(m))) => tcod.mouse = m,
            Some((_, Event::Key(k))) => tcod.key = k,
            _ => tcod.key = Default::default(),
        }

        tcod.con.clear();
        render_all(&mut tcod, &mut game, &objects, fov_recompute);
        tcod.root.flush();

        let player = &mut objects[PLAYER];
        prev_pos = player.pos();
        let action = handle_keys(&mut tcod, &mut game, &mut objects);
        if action == PlayerAction::Exit { break; }

        if objects[PLAYER].alive && action != PlayerAction::DidntTakeTurn {
            for id in 0..objects.len() {
                if objects[id].ai.is_some() {
                    ai_take_turn(id, &tcod, &mut game, &mut objects);
                }
            }
        }
    }
}
