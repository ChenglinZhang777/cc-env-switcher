export type Feedback = { text: string; tone: "success" | "error"; sticky: boolean };

/** 成功消息 2 秒后自动消失。 */
export const FEEDBACK_DISMISS_MS = 2000;

export const successFeedback = (text: string): Feedback => ({ text, tone: "success", sticky: false });

/** 错误消息不自动消失，避免用户没看到就溜走。 */
export const errorFeedback = (text: string): Feedback => ({ text, tone: "error", sticky: true });
