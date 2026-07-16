# Lessons Learned

> **Human-maintained:** Only humans may edit or add content to this file.

1. Games should be replayable entirely deterministically, and replays should be stored.
2. Agents should be able to play and interact with every aspect of the game. Everything I add should be something an agent can interact with, including taking screenshots and videos.
3. I should strictly maintain that the game can be played over an API, through an agent CLI or another agentic interface, and using the UI. Anything added to the UI must also be expanded to the API and CLI, and this should be mechanically enforced.
4. Consider multiple libraries for each core functionality, such as rendering, because migrating off them can be a huge pain. An LLM will pick whatever is most convenient at the start, not necessarily what is best in the long term.
