import {
  CognitoUserPool,
  CognitoUser,
  AuthenticationDetails,
  CognitoUserSession,
} from "amazon-cognito-identity-js";
import { config } from "./config";

const getUserPool = () => {
  if (!config.cognitoUserPoolId || !config.cognitoClientId) {
    throw new Error("Missing Cognito configuration");
  }
  return new CognitoUserPool({
    UserPoolId: config.cognitoUserPoolId,
    ClientId: config.cognitoClientId,
  });
};

const getCurrentUser = () => {
  try {
    return getUserPool().getCurrentUser();
  } catch {
    return null;
  }
};

export const signIn = (username: string, password: string): Promise<CognitoUserSession> => {
  const authenticationDetails = new AuthenticationDetails({
    Username: username,
    Password: password,
  });

  const user = new CognitoUser({
    Username: username,
    Pool: getUserPool(),
  });

  return new Promise((resolve, reject) => {
    user.authenticateUser(authenticationDetails, {
      onSuccess: (session) => resolve(session),
      onFailure: (error) => reject(error),
    });
  });
};

export const signOut = () => {
  const user = getCurrentUser();
  user?.signOut();
};

export const getSession = (): Promise<CognitoUserSession | null> => {
  const user = getCurrentUser();
  if (!user) {
    return Promise.resolve(null);
  }
  return new Promise((resolve) => {
    user.getSession((error: Error | null, session: CognitoUserSession | null) => {
      if (error || !session) {
        resolve(null);
        return;
      }
      resolve(session);
    });
  });
};
