# Lessons Learned

> **Human-maintained:** Only humans may edit or add content to this file.

1. Games should be replayable entirely deterministically, and replays should be stored.
2. Agents should be able to play and interact with every aspect of the game. Everything I add should be something an agent can interact with, including taking screenshots and videos.
3. I should strictly maintain that the game can be played over an API, through an agent CLI or another agentic interface, and using the UI. Anything added to the UI must also be expanded to the API and CLI, and this should be mechanically enforced.
4. Consider multiple libraries for each core functionality, such as rendering, because migrating off them can be a huge pain. An LLM will pick whatever is most convenient at the start, not necessarily what is best in the long term.
5. Gameplay tests must be legible to humans, i.e., dev scenarios. We should be able to spin up tiny games, such as 5-tile-by-5-tile games, for any high-level regression or other test so the player can understand them without reading the code.
6. A server/client architecture is good even if the game is not online because it enforces a boundary between presentation and the game.
7. Everything should be serializable: the interior game state, the state supplied to the renderer, and the camera state. At minimum, render input must be entirely representable independently in memory.
8. Store all game data in files and load it at game-creation time to support modding.
