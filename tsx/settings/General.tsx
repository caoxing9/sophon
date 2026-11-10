// Settings' general section in TSX: theme, language, sending and power.
// The values live in Rust (`src/bridge.rs`); this screen reads them, changes
// them through `Prefs` and hears every change, its own and the Rust UI's.
import { cleanup, signal, View } from "nsh";
import { Prefs, type PrefsState } from "sophon/native";
import { Card, Row, Section, Segmented } from "../kit";

const NONE: PrefsState = { language: "", appearance: "", sendBehavior: "", keepAwake: "", canKeepAwake: false };

/** The preferences, kept current: Rust answers `changed` at once, then at each change. */
function usePrefs() {
  const prefs = signal(NONE);
  let alive = true;
  cleanup(() => (alive = false));
  (async () => {
    let now = NONE;
    while (alive) {
      now = await Prefs.changed(now);
      if (alive) prefs.set(now);
    }
  })();
  return prefs;
}

export function General() {
  const prefs = usePrefs();
  const t = Prefs.t;
  return (
    <View class="gap-8">
      <Section title={t("Appearance")}>
        <Card>
          <Row first title={t("Theme")} hint={t("Light, dark, or as the system is set")}>
            <Segmented
              value={prefs().appearance}
              onChange={Prefs.setAppearance}
              options={[
                { value: "system", label: t("System") },
                { value: "light", label: t("Light") },
                { value: "dark", label: t("Dark") },
              ]}
            />
          </Row>
          <Row title={t("Language")} hint={t("The language of Sophon's interface")}>
            <Segmented
              value={prefs().language}
              onChange={Prefs.setLanguage}
              options={[
                { value: "auto", label: t("System") },
                { value: "en", label: "English" },
                { value: "zh", label: "中文" },
              ]}
            />
          </Row>
        </Card>
      </Section>
      <Section title={t("Sending")}>
        <Card>
          <Row first title={t("While an agent is running")} hint={t("What Return does with your message")}>
            <Segmented
              value={prefs().sendBehavior}
              onChange={Prefs.setSendBehavior}
              options={[
                { value: "steer", label: t("Steer") },
                { value: "interrupt", label: t("Interrupt") },
                { value: "queue", label: t("Queue") },
              ]}
            />
          </Row>
        </Card>
      </Section>
      <If when={prefs().canKeepAwake}>
        <Section title={t("Power")}>
          <Card>
            <Row first title={t("Prevent sleep")} hint={t("Keeps this Mac awake while its agents run")}>
              <Segmented
                value={prefs().keepAwake}
                onChange={Prefs.setKeepAwake}
                options={[
                  { value: "off", label: t("Off") },
                  { value: "on", label: t("On") },
                  { value: "display", label: t("Screen on too") },
                ]}
              />
            </Row>
          </Card>
        </Section>
      </If>
    </View>
  );
}
