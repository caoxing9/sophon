// Sophon's controls for TSX screens, on `nsh/base` and the theme's shadcn
// tokens (`theme.json`): the same look as the Rust kit (`src/ui/kit`).
import { type JSX, Text, View } from "nsh";
import { Tab, Tabs } from "nsh/base";

/** Joins class names, the falsy ones left out. */
export const cn = (...classes: (string | false | null | undefined)[]): string => classes.filter(Boolean).join(" ");

/** A titled group of cards. */
export function Section(props: { title: string; children: JSX.Element }) {
  return (
    <View class="gap-3">
      <Text class="text-xs font-medium text-muted-foreground">{props.title}</Text>
      {props.children}
    </View>
  );
}

/** A flat card of rows. */
export function Card(props: { children: JSX.Element }) {
  return <View class="overflow-hidden rounded-xl border border-border bg-card">{props.children}</View>;
}

/** A settings row: its title and hint, the control after them; a hairline above all but the first. */
export function Row(props: { title: string; hint?: string; first?: boolean; children?: JSX.Element }) {
  return (
    <View class={cn("flex-row items-center gap-4 p-4", !props.first && "border-t border-border")}>
      <View class="flex-1 gap-0.5">
        <Text class="text-sm text-foreground">{props.title}</Text>
        <If when={props.hint}>
          <Text class="text-xs text-muted-foreground">{props.hint}</Text>
        </If>
      </View>
      {props.children}
    </View>
  );
}

/** Segments with a thumb the core springs to the chosen one. */
export function Segmented<T extends string>(props: {
  options: readonly { value: T; label: string }[];
  value: T;
  onChange: (value: T) => void;
}) {
  return (
    <Tabs
      class="h-8 flex-row gap-0.5 self-start rounded-full bg-muted p-[3px]"
      indicatorClass="top-[3px] bottom-[3px] rounded-full border border-border bg-card"
    >
      <Each of={props.options}>
        {(option) => (
          <Tab
            selected={props.value === option().value}
            onPress={() => option().value !== props.value && props.onChange(option().value)}
            class="flex-row items-center justify-center rounded-full border border-transparent px-3"
          >
            <Text class={cn("text-xs font-medium", props.value === option().value ? "text-foreground" : "text-muted-foreground")}>
              {option().label}
            </Text>
          </Tab>
        )}
      </Each>
    </Tabs>
  );
}
