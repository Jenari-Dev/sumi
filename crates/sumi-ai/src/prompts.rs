//! Zero-shot prompts for the CLIP classifier. Each label's prompt embeddings
//! are averaged into one class vector (CLIP's prompt ensembling), so classes
//! with more phrasings get no head start. Naming well-known games helps:
//! CLIP has seen a lot of their screenshots. Embeddings for these exact
//! strings are generated with `sumi-embed-prompts` and shipped in
//! `data/prompt-embeddings.json`.

use crate::decision::Scene;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Label {
    Scene(Scene),
    Game,
    NotGame,
}

use Label::{Game, NotGame};

const FPS: Label = Label::Scene(Scene::Fps);
const MOBA: Label = Label::Scene(Scene::Moba);
const RPG: Label = Label::Scene(Scene::Rpg);
const RACING: Label = Label::Scene(Scene::Racing);
const OFFICE: Label = Label::Scene(Scene::Office);
const SCENERY: Label = Label::Scene(Scene::Scenery);

pub const PROMPTS: &[(Label, &str)] = &[
    (FPS, "a screenshot of a first-person shooter video game"),
    (FPS, "a first-person view holding a gun in a video game"),
    (FPS, "a shooter game with a crosshair in the middle of the screen"),
    (FPS, "gameplay of Counter-Strike 2"),
    (FPS, "gameplay of Call of Duty"),
    (FPS, "gameplay of Apex Legends"),
    (FPS, "gameplay of Valorant"),
    (FPS, "gameplay of Battlefield"),
    (MOBA, "a screenshot of a MOBA game like League of Legends or Dota 2"),
    (MOBA, "a top-down battle arena game with heroes, health bars and a minimap"),
    (MOBA, "gameplay of League of Legends"),
    (MOBA, "gameplay of Dota 2"),
    (MOBA, "an isometric view of heroes fighting in a lane with towers"),
    (RPG, "a screenshot of a role-playing video game"),
    (RPG, "a third-person fantasy adventure game with a character on screen"),
    (RPG, "a screenshot of a real-time strategy game with many units"),
    (RPG, "gameplay of Elden Ring"),
    (RPG, "gameplay of The Witcher 3"),
    (RPG, "gameplay of Baldur's Gate 3"),
    (RPG, "gameplay of Age of Empires"),
    (RPG, "gameplay of an open-world action adventure game"),
    (RACING, "a screenshot of a racing video game"),
    (RACING, "a car driving fast on a race track in a video game"),
    (RACING, "a racing game with a speedometer and lap times"),
    (RACING, "gameplay of Forza Horizon"),
    (RACING, "gameplay of Gran Turismo"),
    (RACING, "a view from behind a car on a road in a driving game"),
    (OFFICE, "a computer desktop with documents and text"),
    (OFFICE, "a web browser showing a website"),
    (OFFICE, "a spreadsheet, a code editor or a chat application"),
    (OFFICE, "a text document in a word processor"),
    (OFFICE, "a terminal window with text"),
    (OFFICE, "a file manager window"),
    (SCENERY, "a landscape photograph of nature"),
    (SCENERY, "a scenic photo of mountains, the sea, a forest or the sky"),
    (SCENERY, "a photograph of a city skyline"),
    (SCENERY, "a nature documentary scene"),
    (SCENERY, "a beautiful sunset over the ocean"),
    (Game, "a screenshot of a video game"),
    (Game, "gameplay footage from a PC game"),
    (Game, "a video game with a heads-up display"),
    (NotGame, "a screenshot of a desktop application"),
    (NotGame, "a photograph"),
    (NotGame, "a frame from a movie or a video"),
    (NotGame, "a website"),
];
