import type { ProfileInfo } from "../bridge";
/** Read-only profile status and its explicit refresh command, independent of React polling. */
export interface ProfileStatusModel {
  supported: boolean;
  profiles: ProfileInfo[] | undefined;
  checking: string[];
  error: string | undefined;
  check(alias: string): Promise<void>;
}
