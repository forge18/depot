# {{project_name}}

A Lapis web application for OpenResty.

## Getting Started

```bash
# Install dependencies
depot install

# Run with OpenResty
openresty -p . -c nginx.conf

# Or use lapis command
lapis server
```

## Project Structure

- `app.lua` - Main Lapis application
- `views/` - ETLua templates
- `static/` - Static files (CSS, JS, images)
- `nginx.conf` - OpenResty/Nginx configuration

## Development

```bash
# Install dependencies
depot install

# Run development server
depot run dev
```

## Resources

- [Lapis Documentation](https://leafo.net/lapis/)
- [OpenResty Documentation](https://openresty.org/en/)

