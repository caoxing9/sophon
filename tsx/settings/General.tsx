// Settings' general section in TSX: theme, language, sending and power.
// The values live in Rust (`src/bridge.rs`); this screen reads them, changes
// them through `Prefs` and hears every change, its own and the Rust UI's.
// Its components are the Rust kit's (`src/ui/kit/exports.rs`), so it looks
// as the Rust settings do.
import { cleanup, signal, View } from "nsh";
import { Card, Prefs, type PrefsState, Section, Segmented, SettingsRow } from "sophon/native";

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
          <SettingsRow first title={t("Theme")} hint={t("Light, dark, or as the system is set")}>
            <Segmented
              value={prefs().appearance}
              onChange={({ value }) => Prefs.setAppearance(value)}
              options={[
                { value: "system", label: t("System") },
                { value: "light", label: t("Light") },
                { value: "dark", label: t("Dark") },
              ]}
            />
          </SettingsRow>
          <SettingsRow title={t("Language")} hint={t("The language of Sophon's interface")}>
            <Segmented
              value={prefs().language}
              onChange={({ value }) => Prefs.setLanguage(value)}
              options={[
                { value: "auto", label: t("System") },
                { value: "en", label: "English" },
                { value: "zh", label: "中文" },
              ]}
            />
          </SettingsRow>
        </Card>
      </Section>
      <Section title={t("Sending")}>
        <Card>
          <SettingsRow first title={t("While an agent is running")} hint={t("What Return does with your message")}>
            <Segmented
              value={prefs().sendBehavior}
              onChange={({ value }) => Prefs.setSendBehavior(value)}
              options={[
                { value: "steer", label: t("Steer") },
                { value: "interrupt", label: t("Interrupt") },
                { value: "queue", label: t("Queue") },
              ]}
            />
          </SettingsRow>
        </Card>
      </Section>
      <If when={prefs().canKeepAwake}>
        <Section title={t("Power")}>
          <Card>
            <SettingsRow first title={t("Prevent sleep")} hint={t("Keeps this Mac awake while its agents run")}>
              <Segmented
                value={prefs().keepAwake}
                onChange={({ value }) => Prefs.setKeepAwake(value)}
                options={[
                  { value: "off", label: t("Off") },
                  { value: "on", label: t("On") },
                  { value: "display", label: t("Screen on too") },
                ]}
              />
            </SettingsRow>
          </Card>
        </Section>
      </If>
    </View>
  );
}
