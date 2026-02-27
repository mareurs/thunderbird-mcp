/* global ExtensionCommon, ChromeUtils, Components, Services, Cc, Ci */
"use strict";

/**
 * Thunderbird MCP — thin XPCOM adapter
 *
 * All MCP protocol logic lives in the Rust binary (thunderbird-mcp).
 * This file exposes a plain HTTP API on localhost:8765 so the Rust
 * binary can call Thunderbird's XPCOM APIs.
 *
 * Output string sanitization (control chars, invalid UTF-8) is handled
 * by the Rust binary's sanitize_str(); no sanitizeForJson() needed here.
 */

const MCP_PORT = 8765;
const AUTH_TOKEN_FILENAME = ".thunderbird-mcp-auth";
const ATTACHMENT_DIR = "/tmp/thunderbird-mcp";
const DEFAULT_MAX_RESULTS = 50;
const MAX_SEARCH_RESULTS_CAP = 200;
const SEARCH_COLLECTION_CAP = 1000;

const resProto = Cc[
  "@mozilla.org/network/protocol;1?name=resource"
].getService(Ci.nsISubstitutingProtocolHandler);

var mcpServer = class extends ExtensionCommon.ExtensionAPI {
  getAPI(context) {
    const extensionRoot = context.extension.rootURI;
    const resourceName = "thunderbird-mcp";

    resProto.setSubstitutionWithFlags(
      resourceName,
      extensionRoot,
      resProto.ALLOW_CONTENT_ACCESS
    );

    return {
      mcpServer: {
        start: async function() {
          if (globalThis.__tbMcpStartPromise) {
            return await globalThis.__tbMcpStartPromise;
          }
          const startPromise = (async () => {
          try {
            const { HttpServer } = ChromeUtils.importESModule(
              "resource://thunderbird-mcp/httpd.sys.mjs?" + Date.now()
            );
            const { NetUtil } = ChromeUtils.importESModule(
              "resource://gre/modules/NetUtil.sys.mjs"
            );
            const { MailServices } = ChromeUtils.importESModule(
              "resource:///modules/MailServices.sys.mjs"
            );

            let cal = null;
            let CalEvent = null;
            try {
              const calModule = ChromeUtils.importESModule(
                "resource:///modules/calendar/calUtils.sys.mjs"
              );
              cal = calModule.cal;
              const { CalEvent: CE } = ChromeUtils.importESModule(
                "resource:///modules/CalEvent.sys.mjs"
              );
              CalEvent = CE;
            } catch {
              // Calendar not available
            }

            // Auth token (XPCOM — Web Crypto API not available in experiment_apis scope)
            const rng = Cc["@mozilla.org/security/random-generator;1"].createInstance(Ci.nsIRandomGenerator);
            const authBytes = rng.generateRandomBytes(32);
            const authToken = Array.from(authBytes, b => b.toString(16).padStart(2, "0")).join("");

            /**
             * CRITICAL: Must specify { charset: "UTF-8" } — NetUtil defaults to Latin-1
             * and will corrupt emojis/non-ASCII characters.
             */
            function readRequestBody(request) {
              const stream = request.bodyInputStream;
              return NetUtil.readInputStreamToString(stream, stream.available(), { charset: "UTF-8" });
            }

            // ── XPCOM helpers ──────────────────────────────────────────────

            function listAccounts() {
              const accounts = [];
              for (const account of MailServices.accounts.accounts) {
                const server = account.incomingServer;
                const identities = [];
                for (const identity of account.identities) {
                  identities.push({
                    id: identity.key,
                    email: identity.email,
                    name: identity.fullName,
                    isDefault: identity === account.defaultIdentity
                  });
                }
                accounts.push({
                  id: account.key,
                  name: server.prettyName,
                  type: server.type,
                  identities
                });
              }
              return accounts;
            }

            function listFolders(accountId, folderPath) {
              const results = [];

              function walkFolder(folder, accountKey, depth) {
                try {
                  const prettyName = folder.prettyName;
                  results.push({
                    name: prettyName || folder.name || "(unnamed)",
                    path: folder.URI,
                    accountId: accountKey,
                    totalMessages: folder.getTotalMessages(false),
                    unreadMessages: folder.getNumUnread(false),
                    depth
                  });
                } catch {
                  // Skip inaccessible folders
                }
                try {
                  if (folder.hasSubFolders) {
                    for (const subfolder of folder.subFolders) {
                      walkFolder(subfolder, accountKey, depth + 1);
                    }
                  }
                } catch {
                  // Skip subfolder traversal errors
                }
              }

              if (folderPath) {
                const folder = MailServices.folderLookup.getFolderForURL(folderPath);
                if (!folder) return { error: `Folder not found: ${folderPath}` };
                const accountKey = folder.server
                  ? (MailServices.accounts.findAccountForServer(folder.server)?.key || "unknown")
                  : "unknown";
                walkFolder(folder, accountKey, 0);
                return results;
              }

              if (accountId) {
                let target = null;
                for (const account of MailServices.accounts.accounts) {
                  if (account.key === accountId) { target = account; break; }
                }
                if (!target) return { error: `Account not found: ${accountId}` };
                try {
                  const root = target.incomingServer.rootFolder;
                  if (root && root.hasSubFolders) {
                    for (const subfolder of root.subFolders) {
                      walkFolder(subfolder, target.key, 0);
                    }
                  }
                } catch {
                  // Skip inaccessible account
                }
                return results;
              }

              for (const account of MailServices.accounts.accounts) {
                try {
                  const root = account.incomingServer.rootFolder;
                  if (!root) continue;
                  if (root.hasSubFolders) {
                    for (const subfolder of root.subFolders) {
                      walkFolder(subfolder, account.key, 0);
                    }
                  }
                } catch {
                  // Skip inaccessible accounts/folders
                }
              }
              return results;
            }

            function findIdentity(emailOrId) {
              if (!emailOrId) return null;
              const lowerInput = emailOrId.toLowerCase();
              for (const account of MailServices.accounts.accounts) {
                for (const identity of account.identities) {
                  if (identity.key === emailOrId || (identity.email || "").toLowerCase() === lowerInput) {
                    return identity;
                  }
                }
              }
              return null;
            }

            const BLOCKED_PATH_PREFIXES = ["/etc/", "/root/", "/proc/", "/sys/", "/dev/"];
            const BLOCKED_PATH_PATTERNS = [/[/.]ssh[/]/, /[/.]gnupg[/]/, /[/.]aws[/]/, /[/.]config[/]gcloud/];

            function addAttachments(composeFields, attachments) {
              const result = { added: 0, failed: [], blocked: [] };
              if (!attachments || !Array.isArray(attachments)) return result;
              for (const filePath of attachments) {
                try {
                  if (BLOCKED_PATH_PREFIXES.some(p => filePath.startsWith(p)) ||
                      BLOCKED_PATH_PATTERNS.some(p => p.test(filePath))) {
                    result.blocked.push(filePath);
                    continue;
                  }
                  const file = Cc["@mozilla.org/file/local;1"].createInstance(Ci.nsIFile);
                  file.initWithPath(filePath);
                  const resolvedPath = file.target || file.path;
                  if (BLOCKED_PATH_PREFIXES.some(p => resolvedPath.startsWith(p)) ||
                      BLOCKED_PATH_PATTERNS.some(p => p.test(resolvedPath))) {
                    result.blocked.push(filePath);
                    continue;
                  }
                  if (file.exists()) {
                    const attachment = Cc["@mozilla.org/messengercompose/attachment;1"]
                      .createInstance(Ci.nsIMsgAttachment);
                    attachment.url = Services.io.newFileURI(file).spec;
                    attachment.name = file.leafName;
                    composeFields.addAttachment(attachment);
                    result.added++;
                  } else {
                    result.failed.push(filePath);
                  }
                } catch {
                  result.failed.push(filePath);
                }
              }
              return result;
            }

            function escapeHtml(s) {
              return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
            }

            function formatBodyHtml(body, isHtml) {
              if (isHtml) {
                let text = (body || "").replace(/\n/g, "");
                text = [...text].map(c => c.codePointAt(0) > 127 ? `&#${c.codePointAt(0)};` : c).join("");
                return text;
              }
              return escapeHtml(body || "").replace(/\n/g, "<br>");
            }

            function setComposeIdentity(msgComposeParams, from, fallbackServer) {
              const identity = findIdentity(from);
              if (identity) {
                msgComposeParams.identity = identity;
                return "";
              }
              if (fallbackServer) {
                const account = MailServices.accounts.findAccountForServer(fallbackServer);
                if (account) msgComposeParams.identity = account.defaultIdentity;
              } else {
                const defaultAccount = MailServices.accounts.defaultAccount;
                if (defaultAccount) msgComposeParams.identity = defaultAccount.defaultIdentity;
              }
              return from ? `unknown identity: ${from}, using default` : "";
            }

            function openFolder(folderPath) {
              try {
                const folder = MailServices.folderLookup.getFolderForURL(folderPath);
                if (!folder) return { error: `Folder not found: ${folderPath}` };
                const isImap = !!(folder.server && folder.server.type === "imap");
                if (isImap) {
                  try { folder.updateFolder(null); } catch (e) {
                    console.debug("IMAP updateFolder failed:", e);
                  }
                }
                const db = folder.msgDatabase;
                if (!db) return { error: "Could not access folder database" };
                return { folder, db, isImap };
              } catch (e) {
                return { error: e.toString() };
              }
            }

            function findMessage(messageId, folderPath) {
              const opened = openFolder(folderPath);
              if (opened.error) return opened;
              const { folder, db, isImap } = opened;
              let msgHdr = null;
              if (typeof db.getMsgHdrForMessageID === "function") {
                try { msgHdr = db.getMsgHdrForMessageID(messageId); } catch { msgHdr = null; }
              }
              if (!msgHdr) {
                for (const hdr of db.enumerateMessages()) {
                  if (hdr.messageId === messageId) { msgHdr = hdr; break; }
                }
              }
              if (!msgHdr) return { error: `Message not found: ${messageId}` };
              return { msgHdr, folder, db, isImap };
            }

            /**
             * Searches all accounts for a message by ID.
             * Used when no folderPath is provided (Rust tools only send message_id).
             */
            function findMessageAnyFolder(messageId) {
              function searchFolderTree(folder) {
                try {
                  const db = folder.msgDatabase;
                  if (db) {
                    let hdr = null;
                    if (typeof db.getMsgHdrForMessageID === "function") {
                      try { hdr = db.getMsgHdrForMessageID(messageId); } catch {}
                    }
                    if (!hdr) {
                      for (const h of db.enumerateMessages()) {
                        if (h.messageId === messageId) { hdr = h; break; }
                      }
                    }
                    if (hdr) return { msgHdr: hdr, folder, db, isImap: !!(folder.server && folder.server.type === "imap") };
                  }
                } catch {}
                if (folder.hasSubFolders) {
                  for (const sub of folder.subFolders) {
                    const r = searchFolderTree(sub);
                    if (r) return r;
                  }
                }
                return null;
              }
              for (const account of MailServices.accounts.accounts) {
                try {
                  const r = searchFolderTree(account.incomingServer.rootFolder);
                  if (r) return r;
                } catch {}
              }
              return { error: `Message not found: ${messageId}` };
            }

            function findTrashFolder(folder) {
              const TRASH_FLAG = 0x00000100;
              try {
                const account = MailServices.accounts.findAccountForServer(folder.server);
                const root = account?.incomingServer?.rootFolder;
                if (!root) return null;
                const stack = [root];
                while (stack.length > 0) {
                  const current = stack.pop();
                  try {
                    if (current && typeof current.getFlag === "function" && current.getFlag(TRASH_FLAG)) {
                      return current;
                    }
                  } catch {}
                  try {
                    if (current && current.hasSubFolders) {
                      for (const sf of current.subFolders) stack.push(sf);
                    }
                  } catch {}
                }
              } catch {}
              return null;
            }

            function searchMessages(query, folderPath, sender, recipient, startDate, endDate, maxResults) {
              const results = [];
              let lowerQuery = (query || "").toLowerCase();
              // Append sender/recipient as search terms if provided
              if (sender) lowerQuery = lowerQuery ? `${lowerQuery} ${sender.toLowerCase()}` : sender.toLowerCase();
              const lowerSender = sender ? sender.toLowerCase() : null;
              const lowerRecipient = recipient ? recipient.toLowerCase() : null;
              const hasQuery = !!(query || sender || recipient);

              const parsedStartDate = startDate ? new Date(startDate).getTime() : NaN;
              const parsedEndDate = endDate ? new Date(endDate).getTime() : NaN;
              const startDateTs = Number.isFinite(parsedStartDate) ? parsedStartDate * 1000 : null;
              const endDateOffset = endDate && !endDate.includes("T") ? 86400000 : 0;
              const endDateTs = Number.isFinite(parsedEndDate) ? (parsedEndDate + endDateOffset) * 1000 : null;
              const requestedLimit = Number(maxResults);
              const effectiveLimit = Math.min(
                Number.isFinite(requestedLimit) && requestedLimit > 0 ? Math.floor(requestedLimit) : DEFAULT_MAX_RESULTS,
                MAX_SEARCH_RESULTS_CAP
              );

              let hasImapFolders = false;

              function searchFolder(folder) {
                if (results.length >= SEARCH_COLLECTION_CAP) return;
                try {
                  if (folder.server && folder.server.type === "imap") {
                    hasImapFolders = true;
                    try { folder.updateFolder(null); } catch (e) {
                      console.debug("IMAP updateFolder failed:", e);
                    }
                  }
                  const db = folder.msgDatabase;
                  if (!db) return;

                  for (const msgHdr of db.enumerateMessages()) {
                    if (results.length >= SEARCH_COLLECTION_CAP) break;

                    const subject = (msgHdr.mime2DecodedSubject || msgHdr.subject || "").toLowerCase();
                    const author = (msgHdr.mime2DecodedAuthor || msgHdr.author || "").toLowerCase();
                    const recipients = (msgHdr.mime2DecodedRecipients || msgHdr.recipients || "").toLowerCase();
                    const ccList = (msgHdr.ccList || "").toLowerCase();
                    const msgDateTs = msgHdr.date || 0;

                    if (startDateTs !== null && msgDateTs < startDateTs) continue;
                    if (endDateTs !== null && msgDateTs > endDateTs) continue;

                    if (lowerSender && !author.includes(lowerSender)) continue;
                    if (lowerRecipient && !recipients.includes(lowerRecipient) && !ccList.includes(lowerRecipient)) continue;

                    const textQuery = (query || "").toLowerCase();
                    if (textQuery && !subject.includes(textQuery) && !author.includes(textQuery) &&
                        !recipients.includes(textQuery) && !ccList.includes(textQuery)) continue;

                    results.push({
                      id: msgHdr.messageId,
                      subject: sanitizeStr(msgHdr.mime2DecodedSubject || msgHdr.subject),
                      author: sanitizeStr(msgHdr.mime2DecodedAuthor || msgHdr.author),
                      recipients: sanitizeStr(msgHdr.mime2DecodedRecipients || msgHdr.recipients),
                      ccList: msgHdr.ccList,
                      date: msgHdr.date ? new Date(msgHdr.date / 1000).toISOString() : null,
                      folder: folder.prettyName,
                      folderPath: folder.URI,
                      read: msgHdr.isRead,
                      flagged: msgHdr.isFlagged,
                      _dateTs: msgDateTs
                    });
                  }
                } catch (e) {
                  console.debug("Skipping inaccessible folder:", folder?.URI, e);
                }

                if (folder.hasSubFolders) {
                  for (const subfolder of folder.subFolders) {
                    if (results.length >= SEARCH_COLLECTION_CAP) break;
                    searchFolder(subfolder);
                  }
                }
              }

              if (folderPath) {
                const folder = MailServices.folderLookup.getFolderForURL(folderPath);
                if (!folder) return { error: `Folder not found: ${folderPath}` };
                searchFolder(folder);
              } else {
                for (const account of MailServices.accounts.accounts) {
                  if (results.length >= SEARCH_COLLECTION_CAP) break;
                  searchFolder(account.incomingServer.rootFolder);
                }
              }

              const seen = new Set();
              const deduped = results.filter(r => { if (!r.id || seen.has(r.id)) return false; seen.add(r.id); return true; });
              deduped.sort((a, b) => b._dateTs - a._dateTs);

              const messages = deduped.slice(0, effectiveLimit).map(r => {
                delete r._dateTs;
                return r;
              });
              if (hasImapFolders) {
                return { messages, imapSyncPending: true, note: "IMAP folder sync is async - results may not include the latest messages. Retry if expected messages are missing." };
              }
              return messages;
            }

            function searchContacts(query, limit) {
              const results = [];
              const lowerQuery = (query || "").toLowerCase();
              const maxResults = limit || DEFAULT_MAX_RESULTS;

              for (const book of MailServices.ab.directories) {
                for (const card of book.childCards) {
                  if (card.isMailList) continue;

                  const email = (card.primaryEmail || "").toLowerCase();
                  const displayName = (card.displayName || "").toLowerCase();
                  const firstName = (card.firstName || "").toLowerCase();
                  const lastName = (card.lastName || "").toLowerCase();

                  if (email.includes(lowerQuery) || displayName.includes(lowerQuery) ||
                      firstName.includes(lowerQuery) || lastName.includes(lowerQuery)) {
                    results.push({
                      id: card.UID,
                      displayName: card.displayName,
                      email: card.primaryEmail || null,
                      firstName: card.firstName,
                      lastName: card.lastName,
                      addressBook: book.dirName
                    });
                  }
                  if (results.length >= maxResults) break;
                }
                if (results.length >= maxResults) break;
              }
              return results;
            }

            function listCalendars() {
              if (!cal) return { error: "Calendar not available" };
              try {
                return cal.manager.getCalendars().map(c => ({
                  id: c.id,
                  name: c.name,
                  type: c.type,
                  readOnly: c.readOnly
                }));
              } catch (e) {
                return { error: e.toString() };
              }
            }

            async function listEvents(calendarId, dateFrom, dateTo, limit) {
              if (!cal) return { error: "Calendar not available" };
              try {
                if (dateFrom && isNaN(new Date(dateFrom).getTime())) return { error: `Invalid date_from: ${dateFrom}` };
                if (dateTo   && isNaN(new Date(dateTo).getTime()))   return { error: `Invalid date_to: ${dateTo}` };

                const dtToISO = (dt) => {
                  if (!dt) return null;
                  try { return cal.dtz.dateTimeToJsDate(dt.getInTimezone(cal.dtz.UTC)).toISOString(); }
                  catch { return dt.icalString; }
                };

                const calendars = cal.manager.getCalendars();
                const targets = calendarId
                  ? calendars.filter(c => c.id === calendarId)
                  : calendars;

                if (calendarId && targets.length === 0) return { error: `Calendar not found: ${calendarId}` };

                const rangeStart = dateFrom ? cal.dtz.jsDateToDateTime(new Date(dateFrom), cal.dtz.UTC) : null;
                const rangeEnd   = dateTo   ? cal.dtz.jsDateToDateTime(new Date(dateTo),   cal.dtz.UTC) : null;
                // ITEM_FILTER_COMPLETED_ALL is a no-op for events but some backends validate the full mask
                const filter = Ci.calICalendar.ITEM_FILTER_TYPE_EVENT | Ci.calICalendar.ITEM_FILTER_COMPLETED_ALL;

                const maxResults = Math.min(
                  Number.isFinite(limit) && limit > 0 ? Math.floor(limit) : DEFAULT_MAX_RESULTS,
                  MAX_SEARCH_RESULTS_CAP
                );

                const safeProp = (item, key) => {
                  try { return item.getProperty(key) || null; } catch { return null; }
                };

                const events = [];
                outer: for (const calendar of targets) {
                  for await (const batch of calendar.getItems(filter, 0, rangeStart, rangeEnd)) {
                    const items = Array.isArray(batch) ? batch : [batch];
                    for (const item of items) {
                      const desc = safeProp(item, "DESCRIPTION");
                      events.push({
                        id: item.id,
                        title: item.title,
                        start: dtToISO(item.startDate),
                        end: dtToISO(item.endDate),
                        location: safeProp(item, "LOCATION"),
                        description: desc ? desc.substring(0, 200) : null,
                        calendar: calendar.name,
                        calendarId: calendar.id,
                      });
                      if (events.length >= maxResults) break outer;
                    }
                  }
                }

                events.sort((a, b) => (a.start || "").localeCompare(b.start || ""));
                return { events, count: events.length };
              } catch (e) {
                return { error: e.toString() };
              }
            }

            function createEvent(title, startDate, endDate, location, description, calendarId, allDay) {
              if (!cal || !CalEvent) return { error: "Calendar module not available" };
              try {
                const win = Services.wm.getMostRecentWindow("mail:3pane");
                if (!win) return { error: "No Thunderbird window found" };

                const startJs = new Date(startDate);
                if (isNaN(startJs.getTime())) return { error: `Invalid startDate: ${startDate}` };

                let endJs = endDate ? new Date(endDate) : null;
                if (endDate && (!endJs || isNaN(endJs.getTime()))) return { error: `Invalid endDate: ${endDate}` };

                if (endJs) {
                  if (allDay) {
                    const startDay = new Date(startJs.getFullYear(), startJs.getMonth(), startJs.getDate());
                    const endDay = new Date(endJs.getFullYear(), endJs.getMonth(), endJs.getDate());
                    if (endDay.getTime() < startDay.getTime()) return { error: "endDate must not be before startDate" };
                  } else if (endJs.getTime() <= startJs.getTime()) {
                    return { error: "endDate must be after startDate" };
                  }
                }

                const event = new CalEvent();
                event.title = title;

                if (allDay) {
                  const startDt = cal.createDateTime();
                  startDt.resetTo(startJs.getFullYear(), startJs.getMonth(), startJs.getDate(), 0, 0, 0, cal.dtz.floating);
                  startDt.isDate = true;
                  event.startDate = startDt;

                  const endDt = cal.createDateTime();
                  if (endJs) {
                    endDt.resetTo(endJs.getFullYear(), endJs.getMonth(), endJs.getDate(), 0, 0, 0, cal.dtz.floating);
                    endDt.isDate = true;
                    if (endDt.compare(startDt) <= 0) {
                      const bumpedEnd = new Date(endJs.getFullYear(), endJs.getMonth(), endJs.getDate());
                      bumpedEnd.setDate(bumpedEnd.getDate() + 1);
                      endDt.resetTo(bumpedEnd.getFullYear(), bumpedEnd.getMonth(), bumpedEnd.getDate(), 0, 0, 0, cal.dtz.floating);
                      endDt.isDate = true;
                    }
                  } else {
                    const defaultEnd = new Date(startJs.getTime());
                    defaultEnd.setDate(defaultEnd.getDate() + 1);
                    endDt.resetTo(defaultEnd.getFullYear(), defaultEnd.getMonth(), defaultEnd.getDate(), 0, 0, 0, cal.dtz.floating);
                    endDt.isDate = true;
                  }
                  event.endDate = endDt;
                } else {
                  event.startDate = cal.dtz.jsDateToDateTime(startJs, cal.dtz.defaultTimezone);
                  if (endJs) {
                    event.endDate = cal.dtz.jsDateToDateTime(endJs, cal.dtz.defaultTimezone);
                  } else {
                    const defaultEnd = new Date(startJs.getTime() + 3600000);
                    event.endDate = cal.dtz.jsDateToDateTime(defaultEnd, cal.dtz.defaultTimezone);
                  }
                }

                if (location) event.setProperty("LOCATION", location);
                if (description) event.setProperty("DESCRIPTION", description);

                const calendars = cal.manager.getCalendars();
                let targetCalendar = null;
                if (calendarId) {
                  targetCalendar = calendars.find(c => c.id === calendarId);
                  if (!targetCalendar) return { error: `Calendar not found: ${calendarId}` };
                  if (targetCalendar.readOnly) return { error: `Calendar is read-only: ${targetCalendar.name}` };
                } else {
                  targetCalendar = calendars.find(c => !c.readOnly);
                  if (!targetCalendar) return { error: "No writable calendar found" };
                }

                event.calendar = targetCalendar;

                const args = {
                  calendarEvent: event,
                  calendar: targetCalendar,
                  mode: "new",
                  inTab: false,
                  onOk(item, calendar) { calendar.addItem(item); },
                };

                win.openDialog(
                  "chrome://calendar/content/calendar-event-dialog.xhtml",
                  "_blank",
                  "centerscreen,chrome,titlebar,toolbar,resizable",
                  args
                );

                return { success: true, message: `Event dialog opened for "${title}" on calendar "${targetCalendar.name}"` };
              } catch (e) {
                return { error: e.toString() };
              }
            }

            function getMessage(messageId, saveAttachments) {
              return new Promise((resolve) => {
                try {
                  const found = findMessageAnyFolder(messageId);
                  if (found.error) { resolve({ error: found.error }); return; }
                  const { msgHdr } = found;

                  const { MsgHdrToMimeMessage } = ChromeUtils.importESModule(
                    "resource:///modules/gloda/MimeMessage.sys.mjs"
                  );

                  MsgHdrToMimeMessage(msgHdr, null, (aMsgHdr, aMimeMsg) => {
                    if (!aMimeMsg) { resolve({ error: "Could not parse message" }); return; }

                    let body = "";
                    let bodyIsHtml = false;
                    try {
                      body = aMimeMsg.coerceBodyToPlaintext();
                    } catch { body = ""; }

                    if (!body) {
                      try {
                        function stripHtml(html) {
                          if (!html) return "";
                          let text = String(html);
                          text = text.replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi, " ");
                          text = text.replace(/<style\b[^>]*>[\s\S]*?<\/style>/gi, " ");
                          text = text.replace(/<br\s*\/?>/gi, "\n");
                          text = text.replace(/<\/(p|div|li|tr|h[1-6]|blockquote|pre)>/gi, "\n");
                          text = text.replace(/<(p|div|li|tr|h[1-6]|blockquote|pre)\b[^>]*>/gi, "\n");
                          text = text.replace(/<[^>]+>/g, " ");
                          const NAMED_ENTITIES = {
                            nbsp: " ", amp: "&", lt: "<", gt: ">", quot: '"', apos: "'", "#39": "'",
                          };
                          text = text.replace(/&(#x?[0-9a-fA-F]+|[a-zA-Z]+);/gi, (match, entity) => {
                            if (entity.startsWith("#x") || entity.startsWith("#X")) {
                              const cp = parseInt(entity.slice(2), 16);
                              return cp ? String.fromCodePoint(cp) : match;
                            }
                            if (entity.startsWith("#")) {
                              const cp = parseInt(entity.slice(1), 10);
                              return cp ? String.fromCodePoint(cp) : match;
                            }
                            return NAMED_ENTITIES[entity.toLowerCase()] || match;
                          });
                          text = text.replace(/\r\n/g, "\n").replace(/\r/g, "\n");
                          text = text.replace(/\n{3,}/g, "\n\n");
                          text = text.replace(/[ \t\f\v]+/g, " ");
                          text = text.replace(/ *\n */g, "\n");
                          return text.trim();
                        }
                        function findBody(part) {
                          const ct = ((part.contentType || "").split(";")[0] || "").trim().toLowerCase();
                          if (ct === "text/plain" && part.body) return { text: part.body, isHtml: false };
                          if (ct === "text/html" && part.body) return { text: part.body, isHtml: true };
                          if (part.parts) {
                            let htmlFallback = null;
                            for (const sub of part.parts) {
                              const r = findBody(sub);
                              if (r && !r.isHtml) return r;
                              if (r && r.isHtml && !htmlFallback) htmlFallback = r;
                            }
                            if (htmlFallback) return htmlFallback;
                          }
                          return null;
                        }
                        const fb = findBody(aMimeMsg);
                        if (fb) {
                          body = fb.isHtml ? stripHtml(fb.text) : fb.text;
                        } else {
                          body = "(Could not extract body text)";
                        }
                      } catch { body = "(Could not extract body text)"; }
                    }

                    const attachments = [];
                    const attachmentSources = [];
                    if (aMimeMsg && aMimeMsg.allUserAttachments) {
                      for (const att of aMimeMsg.allUserAttachments) {
                        const info = {
                          name: att?.name || "",
                          contentType: att?.contentType || "",
                          size: typeof att?.size === "number" ? att.size : null
                        };
                        attachments.push(info);
                        attachmentSources.push({ info, url: att?.url || "", size: typeof att?.size === "number" ? att.size : null });
                      }
                    }

                    const baseResponse = {
                      id: msgHdr.messageId,
                      subject: sanitizeStr(msgHdr.mime2DecodedSubject || msgHdr.subject),
                      author: sanitizeStr(msgHdr.mime2DecodedAuthor || msgHdr.author),
                      recipients: sanitizeStr(msgHdr.mime2DecodedRecipients || msgHdr.recipients),
                      ccList: sanitizeStr(msgHdr.ccList),
                      date: msgHdr.date ? new Date(msgHdr.date / 1000).toISOString() : null,
                      body: sanitizeStr(body),
                      bodyIsHtml,
                      attachments
                    };

                    if (!saveAttachments || attachmentSources.length === 0) {
                      resolve(baseResponse);
                      return;
                    }

                    function sanitizePathSegment(s) {
                      return String(s || "").replace(/[^a-zA-Z0-9]/g, "_") || "message";
                    }
                    function sanitizeFilename(s) {
                      let name = String(s || "").trim();
                      if (!name) name = "attachment";
                      name = name.replace(/[^a-zA-Z0-9._-]/g, "_").replace(/^_+/, "").replace(/_+$/, "");
                      return name || "attachment";
                    }
                    function ensureAttachmentDir(sanitizedId) {
                      const root = Cc["@mozilla.org/file/local;1"].createInstance(Ci.nsIFile);
                      root.initWithPath(ATTACHMENT_DIR);
                      try { root.create(Ci.nsIFile.DIRECTORY_TYPE, 0o755); } catch (e) {
                        if (!root.exists() || !root.isDirectory()) throw e;
                      }
                      const dir = root.clone();
                      dir.append(sanitizedId);
                      try { dir.create(Ci.nsIFile.DIRECTORY_TYPE, 0o755); } catch (e) {
                        if (!dir.exists() || !dir.isDirectory()) throw e;
                      }
                      return dir;
                    }

                    const sanitizedId = sanitizePathSegment(messageId);
                    let dir;
                    try { dir = ensureAttachmentDir(sanitizedId); } catch (e) {
                      for (const { info } of attachmentSources) info.error = `Failed to create attachment directory: ${e}`;
                      resolve(baseResponse);
                      return;
                    }

                    const MAX_ATTACHMENT_BYTES = 50 * 1024 * 1024;

                    const saveOne = ({ info, url, size }, index) => new Promise((done) => {
                      try {
                        if (!url) { info.error = "Missing attachment URL"; done(); return; }
                        const knownSize = typeof size === "number" ? size : null;
                        if (knownSize !== null && knownSize > MAX_ATTACHMENT_BYTES) {
                          info.error = `Attachment too large (${knownSize} bytes, limit ${MAX_ATTACHMENT_BYTES})`;
                          done(); return;
                        }
                        const idx = typeof index === "number" && Number.isFinite(index) ? index : 0;
                        let safeName = sanitizeFilename(info.name);
                        if (!safeName || safeName === "." || safeName === "..") safeName = `attachment_${idx}`;
                        const file = dir.clone();
                        file.append(safeName);
                        try { file.createUnique(Ci.nsIFile.NORMAL_FILE_TYPE, 0o644); } catch (e) {
                          info.error = `Failed to create file: ${e}`; done(); return;
                        }
                        const channel = NetUtil.newChannel({ uri: url, loadUsingSystemPrincipal: true });
                        NetUtil.asyncFetch(channel, (inputStream, status, request) => {
                          try {
                            if (status && status !== 0) {
                              try { inputStream?.close(); } catch {}
                              info.error = `Fetch failed: ${status}`;
                              try { file.remove(false); } catch {}
                              done(); return;
                            }
                            if (!inputStream) {
                              info.error = "Fetch returned no data";
                              try { file.remove(false); } catch {}
                              done(); return;
                            }
                            try {
                              const reqLen = request && typeof request.contentLength === "number" ? request.contentLength : -1;
                              if (reqLen >= 0 && reqLen > MAX_ATTACHMENT_BYTES) {
                                try { inputStream.close(); } catch {}
                                info.error = `Attachment too large (${reqLen} bytes, limit ${MAX_ATTACHMENT_BYTES})`;
                                try { file.remove(false); } catch {}
                                done(); return;
                              }
                            } catch {}
                            const ostream = Cc["@mozilla.org/network/file-output-stream;1"].createInstance(Ci.nsIFileOutputStream);
                            ostream.init(file, -1, -1, 0);
                            NetUtil.asyncCopy(inputStream, ostream, (copyStatus) => {
                              try {
                                if (copyStatus && copyStatus !== 0) {
                                  info.error = `Write failed: ${copyStatus}`;
                                  try { file.remove(false); } catch {}
                                  done(); return;
                                }
                                try {
                                  if (file.fileSize > MAX_ATTACHMENT_BYTES) {
                                    info.error = `Attachment too large (${file.fileSize} bytes, limit ${MAX_ATTACHMENT_BYTES})`;
                                    try { file.remove(false); } catch {}
                                    done(); return;
                                  }
                                } catch {}
                                info.filePath = file.path;
                                done();
                              } catch (e) {
                                info.error = `Write failed: ${e}`;
                                try { file.remove(false); } catch {}
                                done();
                              }
                            });
                          } catch (e) {
                            info.error = `Fetch failed: ${e}`;
                            try { file.remove(false); } catch {}
                            done();
                          }
                        });
                      } catch (e) { info.error = String(e); done(); }
                    });

                    (async () => {
                      try { await Promise.all(attachmentSources.map((src, i) => saveOne(src, i))); } catch (e) {
                        for (const { info } of attachmentSources) {
                          if (!info.error) info.error = `Unexpected save error: ${e}`;
                        }
                      }
                      resolve(baseResponse);
                    })();
                  }, true, { examineEncryptedParts: true });
                } catch (e) { resolve({ error: e.toString() }); }
              });
            }

            function composeMail(to, subject, body, cc, bcc, isHtml, from) {
              try {
                const msgComposeService = Cc["@mozilla.org/messengercompose;1"].getService(Ci.nsIMsgComposeService);
                const msgComposeParams = Cc["@mozilla.org/messengercompose/composeparams;1"].createInstance(Ci.nsIMsgComposeParams);
                const composeFields = Cc["@mozilla.org/messengercompose/composefields;1"].createInstance(Ci.nsIMsgCompFields);

                composeFields.to = Array.isArray(to) ? to.join(", ") : (to || "");
                composeFields.cc = Array.isArray(cc) ? cc.join(", ") : (cc || "");
                composeFields.bcc = Array.isArray(bcc) ? bcc.join(", ") : (bcc || "");
                composeFields.subject = subject || "";

                const formatted = formatBodyHtml(body, isHtml);
                if (isHtml && formatted.includes("<html")) {
                  composeFields.body = formatted;
                } else {
                  composeFields.body = `<html><head><meta charset="UTF-8"></head><body>${formatted}</body></html>`;
                }

                msgComposeParams.type = Ci.nsIMsgCompType.New;
                msgComposeParams.format = Ci.nsIMsgCompFormat.HTML;
                msgComposeParams.composeFields = composeFields;

                const identityWarning = setComposeIdentity(msgComposeParams, from, null);
                msgComposeService.OpenComposeWindowWithParams(null, msgComposeParams);

                let msg = "Compose window opened";
                if (identityWarning) msg += ` (${identityWarning})`;
                return { success: true, message: msg };
              } catch (e) { return { error: e.toString() }; }
            }

            function replyToMessage(messageId, body, replyAll) {
              return new Promise((resolve) => {
                try {
                  const found = findMessageAnyFolder(messageId);
                  if (found.error) { resolve({ error: found.error }); return; }
                  const { msgHdr, folder } = found;

                  const { MsgHdrToMimeMessage } = ChromeUtils.importESModule(
                    "resource:///modules/gloda/MimeMessage.sys.mjs"
                  );

                  MsgHdrToMimeMessage(msgHdr, null, (aMsgHdr, aMimeMsg) => {
                    try {
                      let originalBody = "";
                      if (aMimeMsg) {
                        try { originalBody = aMimeMsg.coerceBodyToPlaintext() || ""; } catch { originalBody = ""; }
                      }

                      const msgComposeService = Cc["@mozilla.org/messengercompose;1"].getService(Ci.nsIMsgComposeService);
                      const msgComposeParams = Cc["@mozilla.org/messengercompose/composeparams;1"].createInstance(Ci.nsIMsgComposeParams);
                      const composeFields = Cc["@mozilla.org/messengercompose/composefields;1"].createInstance(Ci.nsIMsgCompFields);

                      if (replyAll) {
                        composeFields.to = msgHdr.author;
                        const splitAddresses = (s) => (s || "").match(/(?:[^,"]|"[^"]*")+/g) || [];
                        const extractEmail = (s) => (s.match(/<([^>]+)>/)?.[1] || s.trim()).toLowerCase();
                        const ownAccount = MailServices.accounts.findAccountForServer(folder.server);
                        const ownEmail = (ownAccount?.defaultIdentity?.email || "").toLowerCase();
                        const allRecipients = [
                          ...splitAddresses(msgHdr.recipients),
                          ...splitAddresses(msgHdr.ccList)
                        ].map(r => r.trim()).filter(r => r && (!ownEmail || extractEmail(r) !== ownEmail));
                        const seen = new Set();
                        const uniqueRecipients = allRecipients.filter(r => {
                          const email = extractEmail(r);
                          if (seen.has(email)) return false;
                          seen.add(email);
                          return true;
                        });
                        if (uniqueRecipients.length > 0) composeFields.cc = uniqueRecipients.join(", ");
                      } else {
                        composeFields.to = msgHdr.author;
                      }

                      const origSubject = msgHdr.mime2DecodedSubject || msgHdr.subject || "";
                      composeFields.subject = origSubject.startsWith("Re:") ? origSubject : `Re: ${origSubject}`;
                      composeFields.references = `<${messageId}>`;
                      composeFields.setHeader("In-Reply-To", `<${messageId}>`);

                      const dateStr = msgHdr.date ? new Date(msgHdr.date / 1000).toLocaleString() : "";
                      const author = msgHdr.mime2DecodedAuthor || msgHdr.author || "";
                      const quotedLines = originalBody.split("\n").map(line => `&gt; ${escapeHtml(line)}`).join("<br>");
                      const quoteBlock = `<br><br>On ${dateStr}, ${escapeHtml(author)} wrote:<br>${quotedLines}`;

                      composeFields.body = `<html><head><meta charset="UTF-8"></head><body>${formatBodyHtml(body, false)}${quoteBlock}</body></html>`;

                      msgComposeParams.type = Ci.nsIMsgCompType.New;
                      msgComposeParams.format = Ci.nsIMsgCompFormat.HTML;
                      msgComposeParams.composeFields = composeFields;

                      const identityWarning = setComposeIdentity(msgComposeParams, null, folder.server);
                      msgComposeService.OpenComposeWindowWithParams(null, msgComposeParams);

                      let msg = "Reply window opened";
                      if (identityWarning) msg += ` (${identityWarning})`;
                      resolve({ success: true, message: msg });
                    } catch (e) { resolve({ error: e.toString() }); }
                  }, true, { examineEncryptedParts: true });
                } catch (e) { resolve({ error: e.toString() }); }
              });
            }

            function forwardMessage(messageId, to, body) {
              return new Promise((resolve) => {
                try {
                  const found = findMessageAnyFolder(messageId);
                  if (found.error) { resolve({ error: found.error }); return; }
                  const { msgHdr, folder } = found;

                  const { MsgHdrToMimeMessage } = ChromeUtils.importESModule(
                    "resource:///modules/gloda/MimeMessage.sys.mjs"
                  );

                  MsgHdrToMimeMessage(msgHdr, null, (aMsgHdr, aMimeMsg) => {
                    try {
                      const msgComposeService = Cc["@mozilla.org/messengercompose;1"].getService(Ci.nsIMsgComposeService);
                      const msgComposeParams = Cc["@mozilla.org/messengercompose/composeparams;1"].createInstance(Ci.nsIMsgComposeParams);
                      const composeFields = Cc["@mozilla.org/messengercompose/composefields;1"].createInstance(Ci.nsIMsgCompFields);

                      composeFields.to = Array.isArray(to) ? to.join(", ") : (to || "");

                      const origSubject = msgHdr.mime2DecodedSubject || msgHdr.subject || "";
                      composeFields.subject = origSubject.startsWith("Fwd:") ? origSubject : `Fwd: ${origSubject}`;

                      let originalBody = "";
                      if (aMimeMsg) {
                        try { originalBody = aMimeMsg.coerceBodyToPlaintext() || ""; } catch { originalBody = ""; }
                      }

                      const dateStr = msgHdr.date ? new Date(msgHdr.date / 1000).toLocaleString() : "";
                      const fwdAuthor = msgHdr.mime2DecodedAuthor || msgHdr.author || "";
                      const fwdSubject = msgHdr.mime2DecodedSubject || msgHdr.subject || "";
                      const fwdRecipients = msgHdr.mime2DecodedRecipients || msgHdr.recipients || "";
                      const escapedBody = escapeHtml(originalBody).replace(/\n/g, "<br>");

                      const forwardBlock = `-------- Forwarded Message --------<br>` +
                        `Subject: ${escapeHtml(fwdSubject)}<br>Date: ${dateStr}<br>` +
                        `From: ${escapeHtml(fwdAuthor)}<br>To: ${escapeHtml(fwdRecipients)}<br><br>${escapedBody}`;

                      const introHtml = body ? formatBodyHtml(body, false) + "<br><br>" : "";
                      composeFields.body = `<html><head><meta charset="UTF-8"></head><body>${introHtml}${forwardBlock}</body></html>`;

                      let origAttCount = 0;
                      if (aMimeMsg && aMimeMsg.allUserAttachments) {
                        for (const att of aMimeMsg.allUserAttachments) {
                          try {
                            const attachment = Cc["@mozilla.org/messengercompose/attachment;1"].createInstance(Ci.nsIMsgAttachment);
                            attachment.url = att.url;
                            attachment.name = att.name;
                            attachment.contentType = att.contentType;
                            composeFields.addAttachment(attachment);
                            origAttCount++;
                          } catch {}
                        }
                      }

                      msgComposeParams.type = Ci.nsIMsgCompType.New;
                      msgComposeParams.format = Ci.nsIMsgCompFormat.HTML;
                      msgComposeParams.composeFields = composeFields;

                      const identityWarning = setComposeIdentity(msgComposeParams, null, folder.server);
                      msgComposeService.OpenComposeWindowWithParams(null, msgComposeParams);

                      let msg = `Forward window opened with ${origAttCount} attachment(s)`;
                      if (identityWarning) msg += ` (${identityWarning})`;
                      resolve({ success: true, message: msg });
                    } catch (e) { resolve({ error: e.toString() }); }
                  }, true, { examineEncryptedParts: true });
                } catch (e) { resolve({ error: e.toString() }); }
              });
            }

            function getRecentMessages(folderPath, sinceDate, limit, unreadOnly) {
              const results = [];
              let hasImapFolders = false;

              let cutoffTs;
              if (sinceDate) {
                const d = new Date(sinceDate);
                cutoffTs = Number.isFinite(d.getTime()) ? d.getTime() * 1000 : (Date.now() - 7 * 86400000) * 1000;
              } else {
                cutoffTs = (Date.now() - 7 * 86400000) * 1000;
              }

              const requestedLimit = Number(limit);
              const effectiveLimit = Math.min(
                Number.isFinite(requestedLimit) && requestedLimit > 0 ? Math.floor(requestedLimit) : DEFAULT_MAX_RESULTS,
                MAX_SEARCH_RESULTS_CAP
              );

              function collectFromFolder(folder) {
                if (results.length >= SEARCH_COLLECTION_CAP) return;
                try {
                  const db = folder.msgDatabase;
                  if (!db) return;
                  for (const msgHdr of db.enumerateMessages()) {
                    if (results.length >= SEARCH_COLLECTION_CAP) break;
                    const msgDateTs = msgHdr.date || 0;
                    if (msgDateTs < cutoffTs) continue;
                    if (unreadOnly && msgHdr.isRead) continue;
                    results.push({
                      id: msgHdr.messageId,
                      subject: sanitizeStr(msgHdr.mime2DecodedSubject || msgHdr.subject),
                      author: sanitizeStr(msgHdr.mime2DecodedAuthor || msgHdr.author),
                      recipients: sanitizeStr(msgHdr.mime2DecodedRecipients || msgHdr.recipients),
                      date: msgHdr.date ? new Date(msgHdr.date / 1000).toISOString() : null,
                      folder: folder.prettyName,
                      folderPath: folder.URI,
                      read: msgHdr.isRead,
                      flagged: msgHdr.isFlagged,
                      _dateTs: msgDateTs
                    });
                  }
                } catch (e) { console.debug("Skipping inaccessible folder:", folder?.URI, e); }
                if (folder.hasSubFolders) {
                  for (const subfolder of folder.subFolders) {
                    if (results.length >= SEARCH_COLLECTION_CAP) break;
                    collectFromFolder(subfolder);
                  }
                }
              }

              if (folderPath) {
                const opened = openFolder(folderPath);
                if (opened.error) return { error: opened.error };
                if (opened.isImap) hasImapFolders = true;
                collectFromFolder(opened.folder);
              } else {
                for (const account of MailServices.accounts.accounts) {
                  if (results.length >= SEARCH_COLLECTION_CAP) break;
                  try {
                    if (account.incomingServer.type === "imap") hasImapFolders = true;
                    collectFromFolder(account.incomingServer.rootFolder);
                  } catch {}
                }
              }

              const seen = new Set();
              const deduped = results.filter(r => { if (!r.id || seen.has(r.id)) return false; seen.add(r.id); return true; });
              deduped.sort((a, b) => b._dateTs - a._dateTs);
              const messages = deduped.slice(0, effectiveLimit).map(r => { delete r._dateTs; return r; });
              if (hasImapFolders) {
                return { messages, imapSyncPending: true, note: "IMAP folder sync is async - results may not include the latest messages. Retry if expected messages are missing." };
              }
              return messages;
            }

            function deleteMessages(messageIds) {
              try {
                if (typeof messageIds === "string") {
                  try { messageIds = JSON.parse(messageIds); } catch {}
                }
                if (!Array.isArray(messageIds) || messageIds.length === 0) {
                  return { error: "message_ids must be a non-empty array" };
                }

                // Group messages by folder (findMessageAnyFolder for each)
                const byFolder = new Map();
                const notFound = [];
                for (const msgId of messageIds) {
                  if (typeof msgId !== "string" || !msgId) { notFound.push(msgId); continue; }
                  const found = findMessageAnyFolder(msgId);
                  if (found.error) { notFound.push(msgId); continue; }
                  const key = found.folder.URI;
                  if (!byFolder.has(key)) byFolder.set(key, { folder: found.folder, hdrs: [] });
                  byFolder.get(key).hdrs.push(found.msgHdr);
                }

                if (byFolder.size === 0) return { error: "No matching messages found" };

                let totalDeleted = 0;
                const DRAFTS_FLAG = 0x00000400;
                for (const { folder, hdrs } of byFolder.values()) {
                  const isDrafts = typeof folder.getFlag === "function" && folder.getFlag(DRAFTS_FLAG);
                  if (isDrafts) {
                    const trashFolder = findTrashFolder(folder);
                    if (trashFolder) {
                      MailServices.copy.copyMessages(folder, hdrs, trashFolder, true, null, null, false);
                    } else {
                      folder.deleteMessages(hdrs, null, false, true, null, false);
                    }
                  } else {
                    folder.deleteMessages(hdrs, null, false, true, null, false);
                  }
                  totalDeleted += hdrs.length;
                }

                const result = { success: true, deleted: totalDeleted };
                if (notFound.length > 0) result.notFound = notFound;
                return result;
              } catch (e) { return { error: e.toString() }; }
            }

            function updateMessage(messageId, read, flagged, moveTo, trash) {
              try {
                if (typeof messageId !== "string" || !messageId) return { error: "message_id must be a non-empty string" };
                if (read !== undefined) read = read === true || read === "true";
                if (flagged !== undefined) flagged = flagged === true || flagged === "true";
                if (trash !== undefined) trash = trash === true || trash === "true";
                if (moveTo != null && (typeof moveTo !== "string" || !moveTo)) {
                  return { error: "move_to must be a non-empty string" };
                }
                if (moveTo && trash === true) return { error: "Cannot specify both move_to and trash" };

                const found = findMessageAnyFolder(messageId);
                if (found.error) return { error: found.error };

                const { msgHdr, folder } = found;
                const actions = [];

                if (read !== undefined) { msgHdr.markRead(read); actions.push({ type: "read", value: read }); }
                if (flagged !== undefined) { msgHdr.markFlagged(flagged); actions.push({ type: "flagged", value: flagged }); }

                let targetFolder = null;
                if (trash === true) {
                  targetFolder = findTrashFolder(folder);
                  if (!targetFolder) return { error: "Trash folder not found" };
                } else if (moveTo) {
                  targetFolder = MailServices.folderLookup.getFolderForURL(moveTo);
                  if (!targetFolder) return { error: `Folder not found: ${moveTo}` };
                }

                if (targetFolder) {
                  MailServices.copy.copyMessages(folder, [msgHdr], targetFolder, true, null, null, false);
                  actions.push({ type: "move", to: targetFolder.URI });
                }

                return { success: true, actions };
              } catch (e) { return { error: e.toString() }; }
            }

            function createFolder(parentUri, name) {
              try {
                if (typeof parentUri !== "string" || !parentUri) return { error: "parent_uri must be a non-empty string" };
                if (typeof name !== "string" || !name) return { error: "name must be a non-empty string" };

                const parent = MailServices.folderLookup.getFolderForURL(parentUri);
                if (!parent) return { error: `Parent folder not found: ${parentUri}` };

                parent.createSubfolder(name, null);

                let newPath = null;
                try {
                  if (parent.hasSubFolders) {
                    for (const sub of parent.subFolders) {
                      if (sub.prettyName === name || sub.name === name) { newPath = sub.URI; break; }
                    }
                  }
                } catch {}
                if (!newPath) {
                  newPath = parentUri.replace(/\/$/, '') + '/' + name.replace(/ /g, '%20');
                }

                return { success: true, message: `Folder "${name}" created`, path: newPath };
              } catch (e) {
                const msg = e.toString();
                if (msg.includes("NS_MSG_FOLDER_EXISTS")) return { error: `Folder "${name}" already exists under this parent` };
                return { error: msg };
              }
            }

            // ── Filter constants & helpers ──────────────────────────────────

            const ATTRIB_MAP = {
              subject: 0, from: 1, body: 2, date: 3, priority: 4,
              status: 5, to: 6, cc: 7, toOrCc: 8, allAddresses: 9,
              ageInDays: 10, size: 11, tag: 12, hasAttachment: 13,
              junkStatus: 14, junkPercent: 15, otherHeader: 16,
            };
            const ATTRIB_NAMES = Object.fromEntries(Object.entries(ATTRIB_MAP).map(([k, v]) => [v, k]));

            const OP_MAP = {
              contains: 0, doesntContain: 1, is: 2, isnt: 3, isEmpty: 4,
              isBefore: 5, isAfter: 6, isHigherThan: 7, isLowerThan: 8,
              beginsWith: 9, endsWith: 10, isInAB: 11, isntInAB: 12,
              isGreaterThan: 13, isLessThan: 14, matches: 15, doesntMatch: 16,
            };
            const OP_NAMES = Object.fromEntries(Object.entries(OP_MAP).map(([k, v]) => [v, k]));

            const ACTION_MAP = {
              moveToFolder: 0x01, copyToFolder: 0x02, changePriority: 0x03,
              delete: 0x04, markRead: 0x05, killThread: 0x06,
              watchThread: 0x07, markFlagged: 0x08, label: 0x09,
              reply: 0x0A, forward: 0x0B, stopExecution: 0x0C,
              deleteFromServer: 0x0D, leaveOnServer: 0x0E, junkScore: 0x0F,
              fetchBody: 0x10, addTag: 0x11, deleteBody: 0x12,
              markUnread: 0x14, custom: 0x15,
            };
            const ACTION_NAMES = Object.fromEntries(Object.entries(ACTION_MAP).map(([k, v]) => [v, k]));

            function getFilterListForAccount(accountId) {
              const account = MailServices.accounts.getAccount(accountId);
              if (!account) return { error: `Account not found: ${accountId}` };
              const server = account.incomingServer;
              if (!server) return { error: "Account has no server" };
              if (server.canHaveFilters === false) return { error: "Account does not support filters" };
              const filterList = server.getFilterList(null);
              if (!filterList) return { error: "Could not access filter list" };
              return { account, server, filterList };
            }

            function serializeFilter(filter, index) {
              const terms = [];
              try {
                for (const term of filter.searchTerms) {
                  const t = {
                    attrib: ATTRIB_NAMES[term.attrib] || String(term.attrib),
                    op: OP_NAMES[term.op] || String(term.op),
                    booleanAnd: term.booleanAnd,
                  };
                  try {
                    if (term.attrib === 3 || term.attrib === 10) {
                      try {
                        const d = term.value.date;
                        t.value = d ? new Date(d / 1000).toISOString() : (term.value.str || "");
                      } catch { t.value = term.value.str || ""; }
                    } else {
                      t.value = term.value.str || "";
                    }
                  } catch { t.value = ""; }
                  if (term.arbitraryHeader) t.header = term.arbitraryHeader;
                  terms.push(t);
                }
              } catch (e) { console.debug("searchTerms iteration failed:", e); }

              const actions = [];
              for (let a = 0; a < filter.actionCount; a++) {
                try {
                  const action = filter.getActionAt(a);
                  const act = { type: ACTION_NAMES[action.type] || String(action.type) };
                  if (action.type === 0x01 || action.type === 0x02) {
                    act.value = action.targetFolderUri || "";
                  } else if (action.type === 0x03) {
                    act.value = String(action.priority);
                  } else if (action.type === 0x0F) {
                    act.value = String(action.junkScore);
                  } else {
                    try { if (action.strValue) act.value = action.strValue; } catch {}
                  }
                  actions.push(act);
                } catch (e) { console.debug("Skipping unreadable filter action at index", a, e); }
              }

              return {
                index,
                name: filter.filterName,
                enabled: filter.enabled,
                type: filter.filterType,
                temporary: filter.temporary,
                terms,
                actions,
              };
            }

            function sanitizeStr(s) {
              return s ? s.replace(/[\x00-\x08\x0B\x0C\x0E-\x1F\x7F]/g, '') : s;
            }

            function buildTerms(filter, conditions) {
              for (const cond of conditions) {
                const term = filter.createTerm();
                const attribNum = ATTRIB_MAP[cond.attrib] ?? parseInt(cond.attrib);
                if (isNaN(attribNum)) throw new Error(`Unknown attribute: ${cond.attrib}`);
                term.attrib = attribNum;
                const opNum = OP_MAP[cond.op] ?? parseInt(cond.op);
                if (isNaN(opNum)) throw new Error(`Unknown operator: ${cond.op}`);
                term.op = opNum;
                const value = term.value;
                value.attrib = term.attrib;
                value.str = cond.value || "";
                term.value = value;
                term.booleanAnd = cond.booleanAnd !== false;
                if (cond.header) term.arbitraryHeader = cond.header;
                filter.appendTerm(term);
              }
            }

            function buildActions(filter, actions) {
              for (const act of actions) {
                const action = filter.createAction();
                const typeNum = ACTION_MAP[act.type] ?? parseInt(act.type);
                if (isNaN(typeNum)) throw new Error(`Unknown action type: ${act.type}`);
                action.type = typeNum;
                if (act.value) {
                  if (typeNum === 0x01 || typeNum === 0x02) {
                    action.targetFolderUri = act.value;
                  } else if (typeNum === 0x03) {
                    action.priority = parseInt(act.value);
                  } else if (typeNum === 0x0F) {
                    action.junkScore = parseInt(act.value);
                  } else {
                    action.strValue = act.value;
                  }
                }
                filter.appendAction(action);
              }
            }

            function listFilters(accountId) {
              try {
                const results = [];
                let accounts;
                if (accountId) {
                  const account = MailServices.accounts.getAccount(accountId);
                  if (!account) return { error: `Account not found: ${accountId}` };
                  accounts = [account];
                } else {
                  accounts = Array.from(MailServices.accounts.accounts);
                }

                for (const account of accounts) {
                  if (!account) continue;
                  try {
                    const server = account.incomingServer;
                    if (!server || server.canHaveFilters === false) continue;
                    const filterList = server.getFilterList(null);
                    if (!filterList) continue;
                    const filters = [];
                    for (let i = 0; i < filterList.filterCount; i++) {
                      try { filters.push(serializeFilter(filterList.getFilterAt(i), i)); } catch {}
                    }
                    results.push({
                      accountId: account.key,
                      accountName: server.prettyName,
                      filterCount: filterList.filterCount,
                      loggingEnabled: filterList.loggingEnabled,
                      filters,
                    });
                  } catch {}
                }
                return results;
              } catch (e) { return { error: e.toString() }; }
            }

            function createFilter(accountId, name, enabled, type, conditions, actions, insertAtIndex) {
              try {
                if (typeof conditions === "string") { try { conditions = JSON.parse(conditions); } catch {} }
                if (typeof actions === "string") { try { actions = JSON.parse(actions); } catch {} }
                if (typeof enabled === "string") enabled = enabled === "true";
                if (typeof type === "string") type = parseInt(type);
                if (typeof insertAtIndex === "string") insertAtIndex = parseInt(insertAtIndex);
                if (!Array.isArray(conditions) || conditions.length === 0) return { error: "conditions must be a non-empty array" };
                if (!Array.isArray(actions) || actions.length === 0) return { error: "actions must be a non-empty array" };

                const fl = getFilterListForAccount(accountId);
                if (fl.error) return fl;
                const { filterList } = fl;

                const filter = filterList.createFilter(name);
                filter.enabled = enabled !== false;
                filter.filterType = (Number.isFinite(type) && type > 0) ? type : 17;
                buildTerms(filter, conditions);
                buildActions(filter, actions);

                const idx = (insertAtIndex != null && insertAtIndex >= 0)
                  ? Math.min(insertAtIndex, filterList.filterCount)
                  : filterList.filterCount;
                filterList.insertFilterAt(idx, filter);
                filterList.saveToDefaultFile();

                return { success: true, name: filter.filterName, index: idx, filterCount: filterList.filterCount };
              } catch (e) { return { error: e.toString() }; }
            }

            function updateFilter(accountId, filterIndex, name, enabled, type, conditions, actions) {
              try {
                if (typeof filterIndex === "string") filterIndex = parseInt(filterIndex);
                if (!Number.isInteger(filterIndex)) return { error: "filter_index must be an integer" };
                if (typeof enabled === "string") enabled = enabled === "true";
                if (typeof type === "string") type = parseInt(type);
                if (typeof conditions === "string") {
                  try { conditions = JSON.parse(conditions); } catch { return { error: "conditions must be a valid JSON array" }; }
                }
                if (typeof actions === "string") {
                  try { actions = JSON.parse(actions); } catch { return { error: "actions must be a valid JSON array" }; }
                }

                const fl = getFilterListForAccount(accountId);
                if (fl.error) return fl;
                const { filterList } = fl;

                if (filterIndex < 0 || filterIndex >= filterList.filterCount) {
                  return { error: `Invalid filter index: ${filterIndex}` };
                }

                const filter = filterList.getFilterAt(filterIndex);
                const changes = [];

                if (name !== undefined) { filter.filterName = name; changes.push("name"); }
                if (enabled !== undefined) { filter.enabled = enabled; changes.push("enabled"); }
                if (type !== undefined) { filter.filterType = type; changes.push("type"); }

                const replaceConditions = Array.isArray(conditions) && conditions.length > 0;
                const replaceActions = Array.isArray(actions) && actions.length > 0;

                if (replaceConditions || replaceActions) {
                  const newFilter = filterList.createFilter(filter.filterName);
                  newFilter.enabled = filter.enabled;
                  newFilter.filterType = filter.filterType;

                  if (replaceConditions) {
                    buildTerms(newFilter, conditions);
                    changes.push("conditions");
                  } else {
                    let termsCopied = 0;
                    try {
                      for (const term of filter.searchTerms) {
                        const newTerm = newFilter.createTerm();
                        newTerm.attrib = term.attrib;
                        newTerm.op = term.op;
                        const val = newTerm.value;
                        val.attrib = term.attrib;
                        try { val.str = term.value.str || ""; } catch {}
                        try { if (term.attrib === 3) val.date = term.value.date; } catch {}
                        newTerm.value = val;
                        newTerm.booleanAnd = term.booleanAnd;
                        try { newTerm.beginsGrouping = term.beginsGrouping; } catch {}
                        try { newTerm.endsGrouping = term.endsGrouping; } catch {}
                        try { if (term.arbitraryHeader) newTerm.arbitraryHeader = term.arbitraryHeader; } catch {}
                        newFilter.appendTerm(newTerm);
                        termsCopied++;
                      }
                    } catch (e) { return { error: `Failed to copy existing conditions: ${e.toString()}` }; }
                    if (termsCopied === 0) return { error: "Cannot update: failed to read existing filter conditions" };
                  }

                  if (replaceActions) {
                    buildActions(newFilter, actions);
                    changes.push("actions");
                  } else {
                    for (let a = 0; a < filter.actionCount; a++) {
                      try {
                        const origAction = filter.getActionAt(a);
                        const newAction = newFilter.createAction();
                        newAction.type = origAction.type;
                        try { newAction.targetFolderUri = origAction.targetFolderUri; } catch {}
                        try { newAction.priority = origAction.priority; } catch {}
                        try { newAction.strValue = origAction.strValue; } catch {}
                        try { newAction.junkScore = origAction.junkScore; } catch {}
                        newFilter.appendAction(newAction);
                      } catch {}
                    }
                  }

                  filterList.removeFilterAt(filterIndex);
                  filterList.insertFilterAt(filterIndex, newFilter);
                }

                filterList.saveToDefaultFile();
                return { success: true, changes, filter: serializeFilter(filterList.getFilterAt(filterIndex), filterIndex) };
              } catch (e) { return { error: e.toString() }; }
            }

            function deleteFilter(accountId, filterIndex) {
              try {
                if (typeof filterIndex === "string") filterIndex = parseInt(filterIndex);
                if (!Number.isInteger(filterIndex)) return { error: "filter_index must be an integer" };
                const fl = getFilterListForAccount(accountId);
                if (fl.error) return fl;
                const { filterList } = fl;
                if (filterIndex < 0 || filterIndex >= filterList.filterCount) {
                  return { error: `Invalid filter index: ${filterIndex}` };
                }
                const filterName = filterList.getFilterAt(filterIndex).filterName;
                filterList.removeFilterAt(filterIndex);
                filterList.saveToDefaultFile();
                return { success: true, deleted: filterName, remainingCount: filterList.filterCount };
              } catch (e) { return { error: e.toString() }; }
            }

            function reorderFilters(accountId, fromIndex, toIndex) {
              try {
                if (typeof fromIndex === "string") fromIndex = parseInt(fromIndex);
                if (typeof toIndex === "string") toIndex = parseInt(toIndex);
                if (!Number.isInteger(fromIndex)) return { error: "from_index must be an integer" };
                if (!Number.isInteger(toIndex)) return { error: "to_index must be an integer" };

                const fl = getFilterListForAccount(accountId);
                if (fl.error) return fl;
                const { filterList } = fl;

                if (fromIndex < 0 || fromIndex >= filterList.filterCount) return { error: `Invalid source index: ${fromIndex}` };
                if (toIndex < 0 || toIndex >= filterList.filterCount) return { error: `Invalid target index: ${toIndex}` };

                const filter = filterList.getFilterAt(fromIndex);
                filterList.removeFilterAt(fromIndex);
                const adjustedTo = (fromIndex < toIndex) ? toIndex - 1 : toIndex;
                filterList.insertFilterAt(adjustedTo, filter);
                filterList.saveToDefaultFile();

                return { success: true, name: filter.filterName, fromIndex, toIndex };
              } catch (e) { return { error: e.toString() }; }
            }

            function applyFilters(accountId, folderUri) {
              try {
                const fl = getFilterListForAccount(accountId);
                if (fl.error) return fl;
                const { filterList } = fl;

                const folder = MailServices.folderLookup.getFolderForURL(folderUri);
                if (!folder) return { error: `Folder not found: ${folderUri}` };

                let filterService;
                try { filterService = MailServices.filters; } catch {}
                if (!filterService) {
                  try {
                    filterService = Cc["@mozilla.org/messenger/filter-service;1"].getService(Ci.nsIMsgFilterService);
                  } catch {}
                }
                if (!filterService) return { error: "Filter service not available in this Thunderbird version" };

                filterService.applyFiltersToFolders(filterList, [folder], null);

                let enabledCount = 0;
                for (let i = 0; i < filterList.filterCount; i++) {
                  if (filterList.getFilterAt(i).enabled) enabledCount++;
                }
                return {
                  success: true,
                  message: "Filters applied (processing may take a moment)",
                  folder: folderUri,
                  enabledFilters: enabledCount,
                };
              } catch (e) { return { error: e.toString() }; }
            }

            // ── Route table (paths match Rust bridge.rs) ──────────────────

            const ROUTES = {
              "/accounts/list":         async () => listAccounts(),
              "/folders/list":          async ({ account_id, folder_uri }) => listFolders(account_id, folder_uri),
              "/messages/search":       async ({ query, folder, sender, recipient, date_from, date_to, max_results }) =>
                                          searchMessages(query, folder, sender, recipient, date_from, date_to, max_results),
              "/messages/get":          async ({ message_id, save_attachments }) => getMessage(message_id, save_attachments),
              "/messages/recent":       async ({ folder, since_date, limit, unread_only }) =>
                                          getRecentMessages(folder, since_date, limit, unread_only),
              "/messages/update":       async ({ message_id, read, flagged, move_to, trash }) =>
                                          updateMessage(message_id, read, flagged, move_to, trash),
              "/messages/delete":       async ({ message_ids }) => deleteMessages(message_ids),
              "/folders/create":        async ({ parent_uri, name }) => createFolder(parent_uri, name),
              "/mail/send":             async ({ to, subject, body, cc, bcc, from_identity }) =>
                                          composeMail(to, subject, body, cc, bcc, false, from_identity),
              "/mail/reply":            async ({ message_id, body, reply_all }) => replyToMessage(message_id, body, reply_all),
              "/mail/forward":          async ({ message_id, to, body }) => forwardMessage(message_id, to, body),
              "/filters/list":          async ({ account_id }) => listFilters(account_id),
              "/filters/create":        async ({ account_id, name, enabled, type, conditions, actions, insert_at_index }) =>
                                          createFilter(account_id, name, enabled, type, conditions, actions, insert_at_index),
              "/filters/update":        async ({ account_id, filter_index, name, enabled, type, conditions, actions }) =>
                                          updateFilter(account_id, filter_index, name, enabled, type, conditions, actions),
              "/filters/delete":        async ({ account_id, filter_index }) => deleteFilter(account_id, filter_index),
              "/filters/reorder":       async ({ account_id, from_index, to_index }) => reorderFilters(account_id, from_index, to_index),
              "/filters/apply":         async ({ account_id, folder_uri }) => applyFilters(account_id, folder_uri),
              "/contacts/search":       async ({ query, limit }) => searchContacts(query, limit),
              "/calendars/list":        async () => listCalendars(),
              "/calendars/list-events":  async ({ calendar_id, date_from, date_to, limit }) =>
                                           listEvents(calendar_id, date_from, date_to, limit),
              "/calendar/create-event": async ({ calendar_id, title, start, end, description, location }) =>
                                          createEvent(title, start, end, location, description, calendar_id, false),
            };

            // ── HTTP plumbing ──────────────────────────────────────────────

            function makeHandler(fn) {
              return (req, res) => {
                res.processAsync();
                (async () => {
                  try {
                    // Auth check
                    let reqToken = "";
                    try { reqToken = req.getHeader("Authorization"); } catch {}
                    if (reqToken !== `Bearer ${authToken}`) {
                      res.setStatusLine("1.1", 401, "Unauthorized");
                      res.write("Unauthorized");
                      res.finish();
                      return;
                    }

                    // Parse body
                    let params = {};
                    try {
                      const bodyStr = readRequestBody(req);
                      if (bodyStr.trim()) params = JSON.parse(bodyStr);
                    } catch (e) {
                      res.setStatusLine("1.1", 400, "Bad Request");
                      res.setHeader("Content-Type", "application/json; charset=utf-8", false);
                      res.write(JSON.stringify({ error: `Invalid JSON: ${e.message}` }));
                      res.finish();
                      return;
                    }

                    const result = await fn(params);
                    res.setStatusLine("1.1", 200, "OK");
                    // charset=utf-8 is critical — httpd.sys.mjs writes raw bytes
                    res.setHeader("Content-Type", "application/json; charset=utf-8", false);
                    res.write(JSON.stringify(result));
                  } catch (e) {
                    console.error("[thunderbird-mcp] handler error:", e);
                    res.setStatusLine("1.1", 200, "OK");
                    res.setHeader("Content-Type", "application/json; charset=utf-8", false);
                    res.write(JSON.stringify({ error: e.toString() }));
                  }
                  res.finish();
                })();
              };
            }

            const server = new HttpServer();
            for (const [path, fn] of Object.entries(ROUTES)) {
              server.registerPathHandler(path, makeHandler(fn));
            }
            server.start(MCP_PORT);

            // Write auth token only after server.start() succeeds — a failed
            // concurrent start cannot overwrite the token of the running server.
            try {
              const authFile = Cc["@mozilla.org/file/directory_service;1"]
                .getService(Ci.nsIProperties)
                .get("Home", Ci.nsIFile);
              authFile.append(AUTH_TOKEN_FILENAME);
              console.log(`MCP: writing auth token to ${authFile.path}`);
              const foStream = Cc["@mozilla.org/network/file-output-stream;1"]
                .createInstance(Ci.nsIFileOutputStream);
              foStream.init(authFile, 0x02 | 0x08 | 0x20, 0o600, 0);
              const data = authToken + "\n";
              foStream.write(data, data.length);
              foStream.close();
              console.log("MCP: auth token written successfully");
            } catch (e) {
              console.error("MCP: Failed to write auth token file:", e);
            }

            globalThis.__tbMcpServer = server;
            console.log(`Thunderbird MCP adapter listening on port ${MCP_PORT}`);
            return { success: true, port: MCP_PORT };
          } catch (e) {
            if (String(e).includes("NS_ERROR_SOCKET_ADDRESS_IN_USE")) {
              console.log(`MCP: port ${MCP_PORT} already in use — server already running`);
              return { success: true, port: MCP_PORT };
            }
            console.error("Failed to start MCP adapter:", e);
            globalThis.__tbMcpStartPromise = null;
            return { success: false, error: e.toString() };
          }
          })();
          globalThis.__tbMcpStartPromise = startPromise;
          return await startPromise;
        }
      }
    };
  }

  onShutdown(isAppShutdown) {
    if (isAppShutdown) return;
    // NOTE: Do NOT delete the auth token file here.
    // During extension updates, onShutdown(false) for the old version can run
    // AFTER the new version has already written its token.
    try {
      const attachDir = Cc["@mozilla.org/file/local;1"].createInstance(Ci.nsIFile);
      attachDir.initWithPath(ATTACHMENT_DIR);
      if (attachDir.exists() && attachDir.isDirectory()) {
        attachDir.remove(true);
      }
    } catch {}
    resProto.setSubstitution("thunderbird-mcp", null);
    Services.obs.notifyObservers(null, "startupcache-invalidate");
  }
};
