# API Documentation

This document provides technical details for the Register-SPF API.

## Global Standards

### Base URL
All routes are prefixed with `/api`.

### Authentication
Routes marked as **[Auth Required]** expect one of the following headers:
- `Authorization: Bearer <session_token>`
- `X-API-Key: <api_key>`

### Error Responses
All errors follow a unified JSON structure:
```json
{
  "error": "ERROR_CODE",
  "message": "Human readable description",
  "context": { "key": "value" } // Optional
}
```
Common codes: `BAD_REQUEST`, `UNAUTHORIZED`, `FORBIDDEN`, `NOT_FOUND`, `RATE_LIMIT_EXCEEDED`, `DATABASE_ERROR`.

---

## Authentication Routes

### Register
`POST /auth/register`
- **Input**: JSON `RegisterRequest`
  - `username`: 3-30 chars
  - `email`: Valid email string
  - `password`: 8-50 chars
- **Success**: `200 OK` + JSON `AuthResponse`

### Login
`POST /auth/login`
- **Input**: JSON `LoginRequest` (`email`, `password`)
- **Success**: `200 OK` + JSON `AuthResponse`

### List My Tokens [Auth Required]
`GET /auth/tokens`
- **Success**: `200 OK` + JSON `Vec<TokenInfo>`

### Create API Key [Auth Required]
`POST /auth/tokens`
- **Input**: JSON `CreateApiKeyRequest` (`name`)
- **Success**: `200 OK` + JSON `APIKeyResponse`
- **Note**: Maximum of 3 API keys per user.

### Revoke Token [Auth Required]
`DELETE /auth/tokens/{id}`
- **Success**: `204 No Content`

---

## Font Routes

### Create Font [Auth Required]
`POST /fonts/`
- **Input**: `multipart/form-data`
  - `name`: string
  - `slug`: string (unique)
  - `version`: integer
  - `description`: string (optional)
  - `tags`: comma-separated string
  - `file`: .spf binary file
- **Success**: `200 OK` + JSON `FontWithDetails`
- **Limit**: 1 upload per minute (IP and User).

### Search Fonts
`GET /fonts/search`
- **Query Params**:
  - `query`: Search string for font name
  - `tags`: Comma-separated list (e.g., `serif,pixel`)
  - `limit`: integer (default 50)
- **Success**: `200 OK` + JSON `Vec<FontWithDetails>`

### Get Font Details
`GET /fonts/{slug}`
- **Success**: `200 OK` + JSON `FontWithDetails`

### Delete Font [Auth Required]
`DELETE /fonts/{slug}`
- **Success**: `204 No Content`
- **Security**: Only the original uploader can delete a font.

---

## Version Routes

### Create Version [Auth Required]
`POST /fonts/{id}/versions`
- **Input**: `multipart/form-data`
  - `version`: integer
  - `changelog`: string (optional)
  - `file`: .spf binary file
- **Success**: `200 OK` + JSON `FontVersionInfo`
- **Security**: Only the original font uploader can add versions.

### List Versions
`GET /fonts/{id}/versions`
- **Success**: `200 OK` + JSON `Vec<FontVersionInfo>`

---

## Comment Routes

### Add Comment [Auth Required]
`POST /comments/{id}`
- **Path Param**: `id` corresponds to Font ID.
- **Input**: JSON `CreateCommentRequest` (`text`)
- **Success**: `200 OK` + JSON `CommentWithUser`
- **Limit**: 10 comments per minute per user.

### Get Font Comments
`GET /comments/{id}`
- **Path Param**: `id` corresponds to Font ID.
- **Success**: `200 OK` + JSON `Vec<CommentWithUser>`

### Delete Comment [Auth Required]
`DELETE /comments/{id}`
- **Success**: `204 No Content`
- **Security**: Users can only delete their own comments.

---

## Interaction Routes

### Favorite Font [Auth Required]
`POST /fonts/{id}/favorite`
- **Success**: `200 OK`

---

## Rate Limiting Tiers

| Tier | Requests/Min |
| :--- | :--- |
| Anonymous | 10 |
| Authenticated | 40 |
| API Key | 30 |
| Public Frontend | 40 |
| Commenting | 10 |
| Font Upload | 1 |