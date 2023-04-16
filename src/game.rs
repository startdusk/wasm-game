use std::rc::Rc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::channel::mpsc::UnboundedReceiver;
use rand::{thread_rng, Rng};
use web_sys::HtmlImageElement;

use crate::{
    browser,
    engine::{
        self, Audio, Cell, Game, Image, KeyState, Point, Rect, Renderer, Sheet, Sound, SpriteSheet,
    },
    segment::{platform_and_stone, stone_and_platform},
};

use self::red_hat_boy_states::{
    Falling, FallingEndState, Idle, Jumping, JumpingEndState, KnockedOut, RedHatBoyContext,
    RedHatBoyState, Running, Sliding, SlidingEndState,
};

const HEIGHT: i16 = 600;
const TIMELINE_MINIMUM: i16 = 1000;
const OBSTACLE_BUFFER: i16 = 20;

pub struct WalkTheDog {
    machine: Option<WalkTheDogStateMachine>,
}

impl WalkTheDog {
    pub fn new() -> Self {
        WalkTheDog { machine: None }
    }
}

enum WalkTheDogStateMachine {
    Ready(WalkTheDogState<Ready>),
    Walking(WalkTheDogState<Walking>),
    GameOver(WalkTheDogState<GameOver>),
}

impl From<WalkTheDogState<Ready>> for WalkTheDogStateMachine {
    fn from(state: WalkTheDogState<Ready>) -> Self {
        WalkTheDogStateMachine::Ready(state)
    }
}

impl From<WalkTheDogState<Walking>> for WalkTheDogStateMachine {
    fn from(state: WalkTheDogState<Walking>) -> Self {
        WalkTheDogStateMachine::Walking(state)
    }
}

impl From<WalkTheDogState<GameOver>> for WalkTheDogStateMachine {
    fn from(state: WalkTheDogState<GameOver>) -> Self {
        WalkTheDogStateMachine::GameOver(state)
    }
}

impl WalkTheDogStateMachine {
    fn new(walk: Walk) -> Self {
        WalkTheDogStateMachine::Ready(WalkTheDogState::new(walk))
    }

    fn update(self, keystate: &KeyState) -> Self {
        match self {
            WalkTheDogStateMachine::Ready(state) => state.update(keystate).into(),
            WalkTheDogStateMachine::Walking(state) => state.update(keystate).into(),
            WalkTheDogStateMachine::GameOver(state) => state.update(keystate).into(),
        }
    }

    fn draw(&self, renderer: &Renderer) {
        match self {
            WalkTheDogStateMachine::Ready(state) => state.draw(renderer),
            WalkTheDogStateMachine::Walking(state) => state.draw(renderer),
            WalkTheDogStateMachine::GameOver(state) => state.draw(renderer),
        }
    }
}

struct WalkTheDogState<T> {
    walk: Walk,
    state: T,
}

impl<T> WalkTheDogState<T> {
    fn draw(&self, renderer: &Renderer) {
        self.walk.draw(renderer)
    }
}

struct Ready;

impl WalkTheDogState<Ready> {
    fn new(walk: Walk) -> WalkTheDogState<Ready> {
        WalkTheDogState { walk, state: Ready }
    }
    fn update(mut self, keystate: &KeyState) -> ReadyEndState {
        self.walk.boy.update();
        if keystate.is_pressed("ArrowRight") {
            ReadyEndState::Complete(self.start_running())
        } else {
            ReadyEndState::Continue(self)
        }
    }

    fn start_running(mut self) -> WalkTheDogState<Walking> {
        self.run_right();
        WalkTheDogState {
            walk: self.walk,
            state: Walking,
        }
    }

    fn run_right(&mut self) {
        self.walk.boy.run_right();
    }
}

enum ReadyEndState {
    Complete(WalkTheDogState<Walking>),
    Continue(WalkTheDogState<Ready>),
}

impl From<ReadyEndState> for WalkTheDogStateMachine {
    fn from(state: ReadyEndState) -> Self {
        match state {
            ReadyEndState::Complete(walking) => walking.into(),
            ReadyEndState::Continue(ready) => ready.into(),
        }
    }
}

struct Walking;

impl WalkTheDogState<Walking> {
    fn end_game(self) -> WalkTheDogState<GameOver> {
        let receiver = browser::draw_ui("<button id='new_game'>New Game</button>")
            .and_then(|_unit| browser::find_html_element_by_id("new_game"))
            .map(|element| engine::add_click_handler(element))
            .unwrap();

        WalkTheDogState {
            walk: self.walk,
            state: GameOver {
                new_game_event: receiver,
            },
        }
    }

    fn update(mut self, keystate: &KeyState) -> WalkingEndState {
        if keystate.is_pressed("Space") {
            self.walk.boy.jump();
        }

        if keystate.is_pressed("ArrowDown") {
            self.walk.boy.slide();
        }

        self.walk.boy.update();
        let walking_spped = self.walk.velocity();
        let [first_background, second_background] = &mut self.walk.backgrounds;
        first_background.move_horizontally(walking_spped);
        second_background.move_horizontally(walking_spped);

        if first_background.right() < 0 {
            first_background.set_x(second_background.right());
        }

        if second_background.right() < 0 {
            second_background.set_x(first_background.right());
        }

        // retain函数: 保留符合条件的对象
        self.walk.obstacles.retain(|obstacle| obstacle.right() > 0);

        self.walk.obstacles.iter_mut().for_each(|obstacle| {
            obstacle.move_horizontally(walking_spped);
            obstacle.check_intersection(&mut self.walk.boy);
        });

        self.walk.obstacles.iter_mut().for_each(|obstacle| {
            obstacle.move_horizontally(walking_spped);
            obstacle.check_intersection(&mut self.walk.boy);
        });

        if self.walk.timeline < TIMELINE_MINIMUM {
            self.walk.generate_next_segment();
        } else {
            self.walk.timeline += walking_spped;
        }

        if self.walk.knocked_out() {
            WalkingEndState::Complete(self.end_game())
        } else {
            WalkingEndState::Continue(self)
        }
    }
}

enum WalkingEndState {
    Complete(WalkTheDogState<GameOver>),
    Continue(WalkTheDogState<Walking>),
}

impl From<WalkingEndState> for WalkTheDogStateMachine {
    fn from(state: WalkingEndState) -> Self {
        match state {
            WalkingEndState::Complete(walking) => walking.into(),
            WalkingEndState::Continue(ready) => ready.into(),
        }
    }
}
struct GameOver {
    new_game_event: UnboundedReceiver<()>,
}

impl GameOver {
    fn new_game_pressed(&mut self) -> bool {
        matches!(self.new_game_event.try_next(), Ok(Some(())))
    }
}

impl WalkTheDogState<GameOver> {
    fn new_game(self) -> WalkTheDogState<Ready> {
        let _ = browser::hide_ui();
        WalkTheDogState {
            walk: Walk::reset(self.walk),
            state: Ready,
        }
    }

    fn update(mut self, _keystate: &KeyState) -> GameOverEndState {
        if self.state.new_game_pressed() {
            GameOverEndState::Complete(self.new_game())
        } else {
            GameOverEndState::Continue(self)
        }
    }
}

enum GameOverEndState {
    Complete(WalkTheDogState<Ready>),
    Continue(WalkTheDogState<GameOver>),
}

impl From<GameOverEndState> for WalkTheDogStateMachine {
    fn from(state: GameOverEndState) -> Self {
        match state {
            GameOverEndState::Complete(ready) => ready.into(),
            GameOverEndState::Continue(game_over) => game_over.into(),
        }
    }
}

#[async_trait(?Send)]
impl Game for WalkTheDog {
    async fn initialize(&self) -> Result<Box<dyn Game>> {
        match self.machine {
            None => {
                let json = browser::fetch_json("rhb.json").await?;
                let sheet: Sheet = serde_wasm_bindgen::from_value(json)
                    .expect("Could not convert rhb.json into a Sheet structure");

                let image = engine::load_image("rhb.png").await?;
                let audio = Audio::new()?;
                let sound = audio.load_sound("SFX_Jump_23.mp3").await?;
                let backgound_music = audio.load_sound("background_song.mp3").await?;
                audio.play_looping_sound(&backgound_music)?;
                let rhb = RedHatBoy::new(sheet, image, audio, sound);
                let stone = engine::load_image("Stone.png").await?;
                let platform_sheet = browser::fetch_json("tiles.json").await?;
                let platform_sheet: Sheet = serde_wasm_bindgen::from_value(platform_sheet)
                    .expect("Could not convert tiles.json into a Sheet structure");

                let sprite_sheet = Rc::new(SpriteSheet::new(
                    platform_sheet,
                    engine::load_image("tiles.png").await?,
                ));
                let background = engine::load_image("BG.png").await?;
                let background_width = background.width() as i16;

                let starting_obstacles = stone_and_platform(stone.clone(), sprite_sheet.clone(), 0);
                let timeline = right_most(&starting_obstacles);
                let machine = WalkTheDogStateMachine::new(Walk {
                    obstacle_sheet: sprite_sheet.clone(),
                    boy: rhb,
                    backgrounds: [
                        Image::new(background.clone(), Point { x: 0, y: 0 }),
                        Image::new(
                            background,
                            Point {
                                x: background_width,
                                y: 0,
                            },
                        ),
                    ],
                    obstacles: starting_obstacles,
                    stone,
                    timeline,
                });
                Ok(Box::new(WalkTheDog {
                    machine: Some(machine),
                }))
            }
            Some(_) => Err(anyhow!("Error: Game is already initialized")),
        }
    }

    fn update(&mut self, keystate: &KeyState) {
        if let Some(machine) = self.machine.take() {
            self.machine.replace(machine.update(keystate));
        }

        assert!(self.machine.is_some());
    }

    fn draw(&self, renderer: &Renderer) {
        renderer.clear(&Rect {
            position: Point { x: 0, y: 0 },
            width: 600,
            height: 600,
        });

        if let Some(machine) = &self.machine {
            machine.draw(renderer);
        }
    }
}

pub struct Walk {
    obstacle_sheet: Rc<SpriteSheet>,
    boy: RedHatBoy,
    backgrounds: [Image; 2],
    obstacles: Vec<Box<dyn Obstacle>>,

    stone: HtmlImageElement,
    timeline: i16,
}

impl Walk {
    fn reset(walk: Self) -> Self {
        let starting_obstacles =
            stone_and_platform(walk.stone.clone(), walk.obstacle_sheet.clone(), 0);
        let timeline = right_most(&starting_obstacles);

        Walk {
            obstacle_sheet: walk.obstacle_sheet,
            boy: RedHatBoy::reset(walk.boy),
            backgrounds: walk.backgrounds,
            obstacles: starting_obstacles,
            stone: walk.stone,
            timeline,
        }
    }

    fn knocked_out(&self) -> bool {
        self.boy.knocked_out()
    }

    fn velocity(&self) -> i16 {
        -self.boy.walking_speed()
    }

    fn generate_next_segment(&mut self) {
        let mut rng = thread_rng();
        let next_segment = rng.gen_range(0..2);

        let mut next_obstacles = match next_segment {
            0 => stone_and_platform(
                self.stone.clone(),
                self.obstacle_sheet.clone(),
                self.timeline + OBSTACLE_BUFFER,
            ),
            1 => platform_and_stone(
                self.stone.clone(),
                self.obstacle_sheet.clone(),
                self.timeline + OBSTACLE_BUFFER,
            ),
            _ => vec![],
        };

        self.timeline = right_most(&next_obstacles);
        self.obstacles.append(&mut next_obstacles);
    }

    fn draw(&self, renderer: &Renderer) {
        self.backgrounds.iter().for_each(|background| {
            background.draw(renderer);
        });
        self.boy.draw(renderer);
        self.obstacles
            .iter()
            .for_each(|obstacle| obstacle.draw(renderer));
    }
}

pub trait Obstacle {
    /// 检查是否有碰撞
    fn check_intersection(&self, boy: &mut RedHatBoy);

    fn draw(&self, renderer: &Renderer);

    fn move_horizontally(&mut self, x: i16);

    fn right(&self) -> i16;
}

// =============================================================================
// Platform
pub struct Platform {
    sheet: Rc<SpriteSheet>,
    bounding_boxes: Vec<Rect>,
    sprites: Vec<Cell>,
    position: Point,
}

impl Obstacle for Platform {
    fn draw(&self, renderer: &Renderer) {
        let mut x = 0;
        self.sprites.iter().for_each(|sprite| {
            self.sheet.draw(
                renderer,
                &Rect::new_from_x_y(
                    sprite.frame.x,
                    sprite.frame.y,
                    sprite.frame.w,
                    sprite.frame.h,
                ),
                // Just use position and the standard widths in the tileset
                &Rect::new_from_x_y(
                    self.position.x + x,
                    self.position.y,
                    sprite.frame.w,
                    sprite.frame.h,
                ),
            );
            x += sprite.frame.w;
        });
    }

    fn move_horizontally(&mut self, x: i16) {
        self.position.x += x;
        self.bounding_boxes
            .iter_mut()
            .for_each(|bounding_box| bounding_box.set_x(bounding_box.position.x + x))
    }

    fn check_intersection(&self, boy: &mut RedHatBoy) {
        if let Some(box_to_land_on) = self
            .bounding_boxes()
            .iter()
            .find(|&bounding_box| boy.bounding_box().intersects(bounding_box))
        {
            if boy.velocity_y() > 0 && boy.pos_y() < self.position.y {
                boy.land_on(box_to_land_on.y());
            } else {
                boy.knock_out();
            }
        }
    }

    fn right(&self) -> i16 {
        self.bounding_boxes()
            .last()
            .unwrap_or(&Rect::default())
            .right()
    }
}

impl Platform {
    pub fn new(
        sheet: Rc<SpriteSheet>,
        position: Point,
        sprite_names: &[&str],
        bounding_boxes: &[Rect],
    ) -> Self {
        let sprites = sprite_names
            .iter()
            .filter_map(|sprite_name| sheet.cell(sprite_name).cloned())
            .collect();
        let bounding_boxes = bounding_boxes
            .iter()
            .map(|bounding_box| {
                Rect::new_from_x_y(
                    bounding_box.x() + position.x,
                    bounding_box.y() + position.y,
                    bounding_box.width,
                    bounding_box.height,
                )
            })
            .collect();
        Self {
            sheet,
            position,
            sprites,
            bounding_boxes,
        }
    }

    pub fn bounding_boxes(&self) -> &Vec<Rect> {
        &self.bounding_boxes
    }
}

// =============================================================================
// Barrier
pub struct Barrier {
    image: Image,
}

impl Barrier {
    pub fn new(image: Image) -> Self {
        Barrier { image }
    }
}

impl Obstacle for Barrier {
    fn check_intersection(&self, boy: &mut RedHatBoy) {
        if boy.bounding_box().intersects(self.image.bounding_box()) {
            boy.knock_out()
        }
    }

    fn draw(&self, renderer: &Renderer) {
        self.image.draw(renderer);
    }

    fn move_horizontally(&mut self, x: i16) {
        self.image.move_horizontally(x);
    }

    fn right(&self) -> i16 {
        self.image.right()
    }
}

// =============================================================================
// RedHatBoy
pub struct RedHatBoy {
    state_machine: RedHatBoyStateMachine,
    sprite_sheet: Sheet,
    image: HtmlImageElement,
}

impl RedHatBoy {
    fn new(sheet: Sheet, image: HtmlImageElement, audio: Audio, sound: Sound) -> Self {
        Self {
            state_machine: RedHatBoyStateMachine::Idle(RedHatBoyState::new(audio, sound)),
            sprite_sheet: sheet,
            image,
        }
    }

    fn frame_name(&self) -> String {
        format!(
            "{} ({}).png",
            self.state_machine.frame_name(),
            (self.state_machine.context().frame / 3) + 1
        )
    }

    fn current_sprite(&self) -> Option<&Cell> {
        let frame_name = self.frame_name();
        self.sprite_sheet.frames.get(&frame_name)
    }

    fn bounding_box(&self) -> Rect {
        const X_OFFSET: i16 = 18;
        const Y_OFFSET: i16 = 14;
        const WIDTH_OFFSET: i16 = 28;
        let mut bounding_box = self.destination_box();
        bounding_box.set_x(X_OFFSET);
        bounding_box.width -= WIDTH_OFFSET;
        bounding_box.height -= Y_OFFSET;
        bounding_box
    }

    fn destination_box(&self) -> Rect {
        let sprite = self.current_sprite().expect("Cell not found");
        Rect {
            position: Point {
                x: (self.state_machine.context().position.x + sprite.sprite_source_size.x as i16)
                    .into(),
                y: (self.state_machine.context().position.y + sprite.sprite_source_size.y as i16)
                    .into(),
            },
            width: sprite.frame.w.into(),
            height: sprite.frame.h.into(),
        }
    }

    fn draw(&self, renderer: &Renderer) {
        let sprite = self.current_sprite().expect("Cell not found");

        renderer.draw_image(
            &self.image,
            &Rect {
                position: Point {
                    x: sprite.frame.x,
                    y: sprite.frame.y,
                },
                width: sprite.frame.w.into(),
                height: sprite.frame.h.into(),
            },
            &self.destination_box(),
        )
    }

    fn walking_speed(&self) -> i16 {
        self.state_machine.context().velocity.x
    }

    fn update(&mut self) {
        self.state_machine = self.state_machine.clone().update();
    }

    fn run_right(&mut self) {
        self.state_machine = self.state_machine.clone().transition(Event::Run);
    }

    fn slide(&mut self) {
        self.state_machine = self.state_machine.clone().transition(Event::Slide)
    }

    fn jump(&mut self) {
        self.state_machine = self.state_machine.clone().transition(Event::Jump)
    }

    fn knock_out(&mut self) {
        self.state_machine = self.state_machine.clone().transition(Event::KnockOut);
    }

    fn knocked_out(&self) -> bool {
        self.state_machine.knocked_out()
    }

    fn pos_y(&self) -> i16 {
        self.state_machine.context().position.y
    }

    fn velocity_y(&self) -> i16 {
        self.state_machine.context().velocity.y
    }

    fn land_on(&mut self, position: i16) {
        self.state_machine = self.state_machine.clone().transition(Event::Land(position));
    }

    fn reset(boy: Self) -> Self {
        RedHatBoy::new(
            boy.sprite_sheet,
            boy.image,
            boy.state_machine.context().audio.clone(),
            boy.state_machine.context().jump_sound.clone(),
        )
    }
}

// =============================================================================
// RedHatBoyStateMachine
#[derive(Clone)]
enum RedHatBoyStateMachine {
    Idle(RedHatBoyState<Idle>),
    Running(RedHatBoyState<Running>),
    Sliding(RedHatBoyState<Sliding>),
    Jumping(RedHatBoyState<Jumping>),
    Falling(RedHatBoyState<Falling>),
    KnockedOut(RedHatBoyState<KnockedOut>),
}

pub enum Event {
    Run,
    Slide,
    Update,
    Jump,
    KnockOut,
    Land(i16),
}

impl RedHatBoyStateMachine {
    fn transition(self, event: Event) -> Self {
        match (self.clone(), event) {
            // =================================================================
            // to Run
            (RedHatBoyStateMachine::Idle(state), Event::Run) => state.run().into(),

            // =================================================================
            // to Slide
            (RedHatBoyStateMachine::Running(state), Event::Slide) => state.slide().into(),

            // =================================================================
            // to Update
            (RedHatBoyStateMachine::Idle(state), Event::Update) => state.update().into(),
            (RedHatBoyStateMachine::Running(state), Event::Update) => state.update().into(),
            (RedHatBoyStateMachine::Sliding(state), Event::Update) => state.update().into(),
            (RedHatBoyStateMachine::Jumping(state), Event::Update) => state.update().into(),
            (RedHatBoyStateMachine::Falling(state), Event::Update) => state.update().into(),

            // =================================================================
            // to Jump
            (RedHatBoyStateMachine::Running(state), Event::Jump) => state.jump().into(),

            // =================================================================
            // to KnockOut
            (RedHatBoyStateMachine::Sliding(state), Event::KnockOut) => state.knock_out().into(),
            (RedHatBoyStateMachine::Jumping(state), Event::KnockOut) => state.knock_out().into(),
            (RedHatBoyStateMachine::Running(state), Event::KnockOut) => state.knock_out().into(),

            // =================================================================
            // to Land
            (RedHatBoyStateMachine::Jumping(state), Event::Land(position)) => {
                state.land_on(position).into()
            }
            (RedHatBoyStateMachine::Running(state), Event::Land(position)) => {
                state.land_on(position).into()
            }
            (RedHatBoyStateMachine::Sliding(state), Event::Land(position)) => {
                state.land_on(position).into()
            }
            _ => self,
        }
    }

    fn frame_name(&self) -> &str {
        match self {
            RedHatBoyStateMachine::Idle(state) => state.frame_name(),
            RedHatBoyStateMachine::Running(state) => state.frame_name(),
            RedHatBoyStateMachine::Sliding(state) => state.frame_name(),
            RedHatBoyStateMachine::Jumping(state) => state.frame_name(),
            RedHatBoyStateMachine::Falling(state) => state.frame_name(),
            RedHatBoyStateMachine::KnockedOut(state) => state.frame_name(),
        }
    }

    fn context(&self) -> &RedHatBoyContext {
        match self {
            RedHatBoyStateMachine::Idle(state) => &state.context(),
            RedHatBoyStateMachine::Running(state) => &state.context(),
            RedHatBoyStateMachine::Sliding(state) => &state.context(),
            RedHatBoyStateMachine::Jumping(state) => &state.context(),
            RedHatBoyStateMachine::Falling(state) => &state.context(),
            RedHatBoyStateMachine::KnockedOut(state) => &state.context(),
        }
    }

    fn update(self) -> Self {
        self.transition(Event::Update)
    }

    fn knocked_out(&self) -> bool {
        matches!(self, RedHatBoyStateMachine::KnockedOut(_))
    }
}

impl From<RedHatBoyState<Idle>> for RedHatBoyStateMachine {
    fn from(state: RedHatBoyState<Idle>) -> Self {
        RedHatBoyStateMachine::Idle(state)
    }
}

impl From<RedHatBoyState<Running>> for RedHatBoyStateMachine {
    fn from(state: RedHatBoyState<Running>) -> Self {
        RedHatBoyStateMachine::Running(state)
    }
}

impl From<RedHatBoyState<Sliding>> for RedHatBoyStateMachine {
    fn from(state: RedHatBoyState<Sliding>) -> Self {
        RedHatBoyStateMachine::Sliding(state)
    }
}

impl From<RedHatBoyState<Jumping>> for RedHatBoyStateMachine {
    fn from(state: RedHatBoyState<Jumping>) -> Self {
        RedHatBoyStateMachine::Jumping(state)
    }
}

impl From<RedHatBoyState<Falling>> for RedHatBoyStateMachine {
    fn from(state: RedHatBoyState<Falling>) -> Self {
        RedHatBoyStateMachine::Falling(state)
    }
}

impl From<RedHatBoyState<KnockedOut>> for RedHatBoyStateMachine {
    fn from(state: RedHatBoyState<KnockedOut>) -> Self {
        RedHatBoyStateMachine::KnockedOut(state)
    }
}

impl From<SlidingEndState> for RedHatBoyStateMachine {
    fn from(end_state: SlidingEndState) -> Self {
        match end_state {
            SlidingEndState::Running(running) => running.into(),
            SlidingEndState::Sliding(sliding) => sliding.into(),
        }
    }
}

impl From<JumpingEndState> for RedHatBoyStateMachine {
    fn from(end_state: JumpingEndState) -> Self {
        match end_state {
            JumpingEndState::Jumping(jumping_state) => jumping_state.into(),
            JumpingEndState::Loading(loading_state) => loading_state.into(),
        }
    }
}

impl From<FallingEndState> for RedHatBoyStateMachine {
    fn from(state: FallingEndState) -> Self {
        match state {
            FallingEndState::Falling(falling) => falling.into(),
            FallingEndState::KnockedOut(knocked_out) => knocked_out.into(),
        }
    }
}

fn right_most(obstacle_list: &Vec<Box<dyn Obstacle>>) -> i16 {
    obstacle_list
        .iter()
        .map(|obstacle| obstacle.right())
        // the max_by function to figure out the maximum value on the right
        .max_by(|x, y| x.cmp(&y))
        .unwrap_or(0)
}

// =============================================================================
// redharboy states submodules
mod red_hat_boy_states {
    use std::marker;

    use crate::engine::{Audio, Point, Sound};

    use super::HEIGHT;

    const FLOOR: i16 = 480;
    const STARTING_POINT: i16 = -50;

    const IDLE_FRAME_NAME: &str = "Idle";
    const IDLE_FRAMES: u8 = 29;

    const RUN_FRAME_NAME: &str = "Run";
    const RUN_FRAMES: u8 = 23;
    const RUNNING_SPEED: i16 = 3;

    const SLIDING_FRAME_NAME: &str = "Slide";
    const SLIDING_FRAMES: u8 = 14;

    const JUMPING_FRAME_NAME: &str = "Jump";
    const JUMPING_FRAMES: u8 = 35;
    const JUMP_SPEED: i16 = -25;

    const FALLING_FRAMES: u8 = 29; // 10 'Dead' frames in the sheet, * 3 - 1
    const FALLING_FRAME_NAME: &str = "Dead";

    const GRAVITY: i16 = 1;
    const PLAYER_HEIGHT: i16 = HEIGHT - FLOOR;
    const TERMINAL_VELOCITY: i16 = 20;

    // =========================================================================
    // Idle
    #[derive(Clone, Copy)]
    pub struct Idle;

    impl RedHatBoyState<Idle> {
        pub fn new(audio: Audio, jump_sound: Sound) -> Self {
            RedHatBoyState {
                ctx: RedHatBoyContext {
                    frame: 0,
                    position: Point {
                        x: STARTING_POINT,
                        y: FLOOR,
                    },
                    velocity: Point { x: 0, y: 0 },
                    audio,
                    jump_sound,
                },
                _state: marker::PhantomData,
            }
        }

        pub fn run(self) -> RedHatBoyState<Running> {
            RedHatBoyState {
                ctx: self.ctx.reset_frame().run_right(),
                _state: marker::PhantomData,
            }
        }

        pub fn frame_name(&self) -> &str {
            IDLE_FRAME_NAME
        }

        pub fn update(mut self) -> Self {
            self.update_context(IDLE_FRAMES);
            self
        }
    }

    // =========================================================================
    // Sliding
    pub enum SlidingEndState {
        Running(RedHatBoyState<Running>),
        Sliding(RedHatBoyState<Sliding>),
    }

    #[derive(Clone, Copy)]
    pub struct Sliding;

    impl RedHatBoyState<Sliding> {
        pub fn frame_name(&self) -> &str {
            SLIDING_FRAME_NAME
        }

        pub fn update(mut self) -> SlidingEndState {
            self.update_context(SLIDING_FRAMES);
            if self.ctx.frame >= SLIDING_FRAMES {
                SlidingEndState::Running(self.stand())
            } else {
                SlidingEndState::Sliding(self)
            }
        }

        pub fn stand(self) -> RedHatBoyState<Running> {
            RedHatBoyState {
                ctx: self.ctx.reset_frame(),
                _state: marker::PhantomData,
            }
        }

        pub fn knock_out(self) -> RedHatBoyState<Falling> {
            RedHatBoyState {
                ctx: self.ctx.reset_frame().stop(),
                _state: marker::PhantomData,
            }
        }

        pub fn land_on(self, position: i16) -> RedHatBoyState<Running> {
            RedHatBoyState {
                ctx: self.ctx.set_on(position),
                _state: marker::PhantomData,
            }
        }
    }

    // =========================================================================
    // Running
    #[derive(Clone, Copy)]
    pub struct Running;

    impl RedHatBoyState<Running> {
        pub fn frame_name(&self) -> &str {
            RUN_FRAME_NAME
        }

        pub fn update(mut self) -> Self {
            self.update_context(RUN_FRAMES);
            self
        }

        pub fn slide(self) -> RedHatBoyState<Sliding> {
            RedHatBoyState {
                ctx: self.ctx.reset_frame(),
                _state: marker::PhantomData,
            }
        }

        pub fn jump(self) -> RedHatBoyState<Jumping> {
            RedHatBoyState {
                ctx: self
                    .ctx
                    .reset_frame()
                    .set_vertical_velocity(JUMP_SPEED)
                    .play_jump_sound(),
                _state: marker::PhantomData,
            }
        }

        pub fn knock_out(self) -> RedHatBoyState<Falling> {
            RedHatBoyState {
                ctx: self.ctx.reset_frame().stop(),
                _state: marker::PhantomData,
            }
        }

        pub fn land_on(self, position: i16) -> RedHatBoyState<Running> {
            RedHatBoyState {
                ctx: self.ctx.set_on(position),
                _state: marker::PhantomData,
            }
        }
    }

    // =========================================================================
    // Jumping
    pub enum JumpingEndState {
        Loading(RedHatBoyState<Running>),
        Jumping(RedHatBoyState<Jumping>),
    }

    #[derive(Clone, Copy)]
    pub struct Jumping;

    impl RedHatBoyState<Jumping> {
        pub fn frame_name(&self) -> &str {
            JUMPING_FRAME_NAME
        }

        pub fn update(mut self) -> JumpingEndState {
            self.update_context(JUMPING_FRAMES);
            if self.ctx.position.y >= FLOOR {
                JumpingEndState::Loading(self.land_on(HEIGHT.into()))
            } else {
                JumpingEndState::Jumping(self)
            }
        }

        pub fn knock_out(self) -> RedHatBoyState<Falling> {
            RedHatBoyState {
                ctx: self.ctx.reset_frame().stop(),
                _state: marker::PhantomData,
            }
        }

        pub fn land_on(self, position: i16) -> RedHatBoyState<Running> {
            RedHatBoyState {
                ctx: self.ctx.reset_frame().set_on(position as i16),
                _state: marker::PhantomData,
            }
        }
    }

    // =========================================================================
    // Falling
    pub enum FallingEndState {
        KnockedOut(RedHatBoyState<KnockedOut>),
        Falling(RedHatBoyState<Falling>),
    }

    #[derive(Clone, Copy)]
    pub struct Falling;

    impl RedHatBoyState<Falling> {
        pub fn frame_name(&self) -> &str {
            FALLING_FRAME_NAME
        }

        pub fn knock_out(self) -> RedHatBoyState<KnockedOut> {
            RedHatBoyState {
                ctx: self.ctx,
                _state: marker::PhantomData,
            }
        }

        pub fn update(mut self) -> FallingEndState {
            self.update_context(FALLING_FRAMES);
            if self.ctx.frame >= FALLING_FRAMES {
                FallingEndState::KnockedOut(self.knock_out())
            } else {
                FallingEndState::Falling(self)
            }
        }
    }

    // =========================================================================
    // KnockedOut
    #[derive(Copy, Clone)]
    pub struct KnockedOut;

    impl RedHatBoyState<KnockedOut> {
        pub fn frame_name(&self) -> &str {
            FALLING_FRAME_NAME
        }
    }

    #[derive(Clone)]
    pub struct RedHatBoyState<S> {
        pub ctx: RedHatBoyContext,
        // PhantomData<T>是一个零大小类型的标记结构体
        // 作用:
        //      并不使用的类型;
        //      型变;
        //      标记拥有关系;
        //      自动trait实现(send/sync);
        _state: marker::PhantomData<S>,
    }

    impl<S> RedHatBoyState<S> {
        pub fn context(&self) -> &RedHatBoyContext {
            &self.ctx
        }

        fn update_context(&mut self, frames: u8) {
            self.ctx = self.ctx.clone().update(frames)
        }
    }

    #[derive(Clone)]
    pub struct RedHatBoyContext {
        pub frame: u8,
        pub position: Point,
        pub velocity: Point,
        pub audio: Audio,
        pub jump_sound: Sound,
    }

    impl RedHatBoyContext {
        fn update(mut self, frame_count: u8) -> Self {
            if self.velocity.y < TERMINAL_VELOCITY {
                self.velocity.y += GRAVITY;
            }
            if self.frame < frame_count {
                self.frame += 1;
            } else {
                self.frame = 0;
            }

            // self.position.x += self.velocity.x; // 横版游戏，人物在原地，物体相对于人跑过来，所以任务不需要移动
            self.position.y += self.velocity.y;

            if self.position.y > FLOOR {
                self.position.y = FLOOR;
            }

            self
        }

        fn reset_frame(mut self) -> Self {
            self.frame = 0;
            self
        }

        fn run_right(mut self) -> Self {
            self.velocity.x += RUNNING_SPEED;
            self
        }

        fn set_vertical_velocity(mut self, y: i16) -> Self {
            self.velocity.y = y;
            self
        }

        fn stop(mut self) -> Self {
            self.velocity.x = 0;
            self.velocity.y = 0;
            self
        }

        fn set_on(mut self, position: i16) -> Self {
            let position = position - PLAYER_HEIGHT;
            self.position.y = position;
            self
        }

        fn play_jump_sound(self) -> Self {
            if let Err(err) = self.audio.play_sound(&self.jump_sound) {
                log!("Error playing jump sound {:#?}", err)
            }
            self
        }
    }
}
