import { useAppDispatch, useAppState } from "../AppContext";
import { ActionType, Phase } from "../types";
import { MOCK_PHASE_ACTIONS } from "./devMocks";

const DEV_PHASES = Object.values(Phase).filter(
  (p) => p !== Phase.Connecting,
) as Phase[];

export function DevToolbar() {
  const state = useAppState();
  const dispatch = useAppDispatch();

  function goToPhase(phase: Phase) {
    // Reset to clean slate
    dispatch({ type: ActionType.Logout });
    // Replay the action chain for this phase
    const actions = MOCK_PHASE_ACTIONS[phase]();
    for (const action of actions) {
      dispatch(action);
    }
  }

  return (
    <div className="bottom-0 left-0 right-0 bg-bg1 border-t border-border px-2 py-1 flex gap-1 flex-wrap z-50">      <span className="text-2xs text-dim font-mono tracking-wide self-center mr-1">
        DEV
      </span>
      {DEV_PHASES.map((p) => (
        <button
          key={p}
          onClick={() => goToPhase(p)}
          className={`
            text-2xs font-mono tracking-wide px-2 py-0.5 rounded cursor-pointer border transition-colors
            ${
              state.phase === p
                ? "bg-orange text-white border-orange"
                : "bg-transparent text-muted border-border hover:border-muted hover:text-text"
            }
          `}
        >
          {p}
        </button>
      ))}
    </div>
  );
}
