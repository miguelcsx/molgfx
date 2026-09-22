// The bounded typed-bytecode interpreter for visual programs.
//
// This is an explicit, reported fallback rather than a leftover: a program
// whose opcodes the straight-line emitter does not lower, or whose compiled
// specialization failed, runs this loop permanently for that style. The
// register file is the same width and the same zero initialization in both
// strategies, and both hand it to `visual_resolve_registers`, so the two paths
// resolve identical pixels from the same uniform.

fn visual_resolve(
    inputs: VisualEvaluationInputs,
    fallback: VisualFragmentResult,
) -> VisualFragmentResult {
    if !VISUAL_FRAGMENT_ENABLED || visual_config.counts.z == 0u {
        return fallback;
    }
    var registers: array<vec4f, 64>;
    for (var index = 0u; index < visual_config.counts.x; index++) {
        registers[index] = visual_evaluate_instruction(
            visual_instruction(index),
            inputs,
            &registers,
        );
    }
    return visual_resolve_registers(&registers, fallback);
}
