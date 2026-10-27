import type { Messages } from "..";
import type en from "../en";
import cards from "./cards";
import common from "./common";
import dashboard from "./dashboard";
import detail from "./detail";
import ebay from "./ebay";
import market from "./market";
import models from "./models";
import newSession from "./newSession";
import session from "./session";
import sessions from "./sessions";
import settings from "./settings";
import sidebar from "./sidebar";
import status from "./status";
import wizard from "./wizard";

const messages: Messages<typeof en> = {
  common,
  wizard,
  status,
  sidebar,
  dashboard,
  sessions,
  session,
  cards,
  detail,
  newSession,
  settings,
  market,
  ebay,
  models,
};
export default messages;
