# Player report context

The original player report text is not preserved as a separate raw chat transcript in this evidence
directory. The packaging request provided the symptoms to retain, so this file records those phrases
and the interpretation boundary.

User-supplied symptom phrases:

- "laggy"
- "visual stuttering"
- "RTT high"
- "JIT high"

Later reproduction context supplied with the task:

- `jit` rises during high-density commands.
- The `jit` rise stops when letting the game run without issuing dense commands.

Interpretation note: the reported "JIT" is the in-game HUD `jit` field, meaning snapshot arrival
jitter. It is not JavaScript compiler JIT behavior.
