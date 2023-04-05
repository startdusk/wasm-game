use anyhow::{anyhow, Result};
use async_trait::async_trait;
use web_sys::HtmlImageElement;

use crate::{
    browser,
    engine::{self, Cell, Game, Image, KeyState, Point, Rect, Renderer, Sheet},
};

use self::red_hat_boy_states::{
    Falling, FallingEndState, Idle, Jumping, JumpingEndState, KnockedOut, RedHatBoyContext,
    RedHatBoyState, Running, Sliding, SlidingEndState,
};

pub enum WalkTheDog {
    Loading,
    Loaded(Walk),
}

impl WalkTheDog {
    pub fn new() -> WalkTheDog {
        WalkTheDog::Loading
    }
}

#[async_trait(?Send)]
impl Game for WalkTheDog {
    async fn initialize(&self) -> Result<Box<dyn Game>> {
        match self {
            WalkTheDog::Loading => {
                let json = browser::fetch_json("rhb.json").await?;
                let sheet: Sheet = serde_wasm_bindgen::from_value(json)
                    .expect("Could not convert rhb.json into a Sheet structure");

                let image = engine::load_image("rhb.png").await?;
                let rhb = RedHatBoy::new(sheet, image);
                let background = engine::load_image("BG.png").await?;
                let stone = engine::load_image("Stone.png").await?;
                Ok(Box::new(WalkTheDog::Loaded(Walk {
                    boy: rhb,
                    background: Image::new(background, Point { x: 0, y: 0 }),
                    stone: Image::new(stone, Point { x: 150, y: 546 }),
                })))
            }
            WalkTheDog::Loaded(_) => Err(anyhow!("Error: Game is already initialized")),
        }
    }

    fn update(&mut self, keystate: &KeyState) {
        if let WalkTheDog::Loaded(walk) = self {
            if keystate.is_pressed("ArrowDown") {
                walk.boy.slide();
            }

            if keystate.is_pressed("ArrowRight") {
                walk.boy.run_right();
            }

            if keystate.is_pressed("Space") {
                walk.boy.jump();
            }
            walk.boy.update();
            if walk
                .boy
                .bounding_box()
                .intersects(walk.stone.bounding_box())
            {
                walk.boy.knock_out();
            }
        }
    }

    fn draw(&self, renderer: &Renderer) {
        renderer.clear(&Rect {
            x: 0.0,
            y: 0.0,
            width: 600.0,
            height: 600.0,
        });

        if let WalkTheDog::Loaded(walk) = self {
            walk.background.draw(renderer);
            walk.boy.draw(renderer);
            walk.stone.draw(renderer);
        }
    }
}

pub struct Walk {
    boy: RedHatBoy,
    background: Image,
    stone: Image,
}

pub struct RedHatBoy {
    state_machine: RedHatBoyStateMachine,
    sprite_sheet: Sheet,
    image: HtmlImageElement,
}

impl RedHatBoy {
    fn new(sheet: Sheet, image: HtmlImageElement) -> Self {
        Self {
            state_machine: RedHatBoyStateMachine::Idle(RedHatBoyState::new()),
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
        let sprite = self.current_sprite().expect("Cell not found");
        Rect {
            x: (self.state_machine.context().position.x + sprite.sprite_source_size.x as i16)
                .into(),
            y: (self.state_machine.context().position.y + sprite.sprite_source_size.y as i16)
                .into(),
            width: sprite.frame.w.into(),
            height: sprite.frame.h.into(),
        }
    }

    fn draw(&self, renderer: &Renderer) {
        let sprite = self.current_sprite().expect("Cell not found");

        renderer.draw_image(
            &self.image,
            &Rect {
                x: sprite.frame.x.into(),
                y: sprite.frame.y.into(),
                width: sprite.frame.w.into(),
                height: sprite.frame.h.into(),
            },
            &self.bounding_box(),
        )
    }

    fn update(&mut self) {
        self.state_machine = self.state_machine.update();
    }

    fn run_right(&mut self) {
        self.state_machine = self.state_machine.transition(Event::Run);
    }

    fn slide(&mut self) {
        self.state_machine = self.state_machine.transition(Event::Slide)
    }

    fn jump(&mut self) {
        self.state_machine = self.state_machine.transition(Event::Jump)
    }

    fn knock_out(&mut self) {
        self.state_machine = self.state_machine.transition(Event::KnockOut);
    }
}

#[derive(Clone, Copy)]
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
            SlidingEndState::Complete(running_state) => running_state.into(),
            SlidingEndState::Sliding(sliding_state) => sliding_state.into(),
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

impl RedHatBoyStateMachine {
    fn transition(self, event: Event) -> Self {
        match (self, event) {
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
}

mod red_hat_boy_states {
    use std::marker;

    use crate::engine::Point;

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

    // =========================================================================
    // Idle
    #[derive(Clone, Copy)]
    pub struct Idle;

    impl RedHatBoyState<Idle> {
        pub fn new() -> Self {
            RedHatBoyState {
                ctx: RedHatBoyContext {
                    frame: 0,
                    position: Point {
                        x: STARTING_POINT,
                        y: FLOOR,
                    },
                    velocity: Point { x: 0, y: 0 },
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
        Complete(RedHatBoyState<Running>),
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
                SlidingEndState::Complete(self.stand())
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
                ctx: self.ctx.set_vertical_velocity(JUMP_SPEED),
                _state: marker::PhantomData,
            }
        }

        pub fn knock_out(self) -> RedHatBoyState<Falling> {
            RedHatBoyState {
                ctx: self.ctx.reset_frame().stop(),
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
                JumpingEndState::Loading(self.load())
            } else {
                JumpingEndState::Jumping(self)
            }
        }

        pub fn load(self) -> RedHatBoyState<Running> {
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

    #[derive(Clone, Copy)]
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
            self.ctx = self.ctx.update(frames)
        }
    }

    #[derive(Clone, Copy)]
    pub struct RedHatBoyContext {
        pub frame: u8,
        pub position: Point,
        pub velocity: Point,
    }

    impl RedHatBoyContext {
        fn update(mut self, frame_count: u8) -> Self {
            self.velocity.y += GRAVITY;
            if self.frame < frame_count {
                self.frame += 1;
            } else {
                self.frame = 0;
            }
            self.position.x += self.velocity.x;
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
    }
}
