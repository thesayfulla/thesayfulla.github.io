"""Resume content for /cv/. Edit this file to update the page.

Each bullet is (bold lead, rest of the sentence).
"""

TITLE = "Software Engineer"

SUMMARY = (
    "Software Engineer focused on backend and infrastructure, with <b>4+ years of experience</b> "
    "building B2B products, production APIs, AI services, and cloud infrastructure with Python, "
    "Django, PostgreSQL, Docker, AWS, and LLMs."
)

EXPERIENCE = [
    {
        "company": "WordExpert AI",
        "url": "https://wordexpert.ai/",
        "role": "Software Engineer",
        "dates": "Mar 2025 – Feb 2026",
        "about": "Multi-site SaaS platform serving Dashboard, Freelance, and Agency/Business workflows.",
        "bullets": [
            ("Delivered scalable backend APIs", "for multiple SaaS products using Django REST Framework and ML model serving."),
            ("Led the frontend implementation,", "building Vue.js interfaces for Dashboard, Freelance, and Agency products."),
            ("Reduced unnecessary database queries", "by eliminating N+1 query patterns across Django applications."),
            ("Improved backend reliability", "by diagnosing production issues, fixing bugs, and refactoring existing services."),
            ("Automated application delivery", "with GitHub Actions CI/CD pipelines from development through deployment."),
            ("Expanded language-processing capabilities", "by evaluating and integrating NLP libraries for text features."),
        ],
    },
    {
        "company": "AIsha Group",
        "url": "",
        "role": "Software Engineer",
        "dates": "Oct 2024 – Jan 2025",
        "about": "B2B engineering team building AI-powered products and ML services.",
        "bullets": [
            ("Improved PostgreSQL performance by 2.5x", "by migrating JSON workloads to JSONB and optimizing access patterns."),
            ("Automated payment and account synchronization", "with APIs linking backend services to Telegram workflows."),
            ("Integrated multiple LLM providers", "(ChatGPT, Gemini, TogetherAI), making models switchable through backend APIs."),
            ("Built AI retrieval workflows", "by implementing RAG pipelines on vector databases for LLM-powered applications."),
            ("Improved infrastructure observability", "with Grafana, Prometheus, and GPU Exporter for server and GPU metrics."),
            ("Added self-hosted object storage", "by deploying MinIO as an S3-compatible storage layer."),
            ("Deployed backend services on AWS EC2,", "managing application infrastructure and production releases."),
        ],
    },
    {
        "company": "Unione",
        "url": "",
        "role": "Co-Founder & Engineer",
        "dates": "Sep 2023 – May 2024",
        "about": "University platform containing approximately 4,000 majors, quizzes, and educational datasets.",
        "bullets": [
            ("Built the core backend platform", "with Django REST APIs for university filtering, mock exams, and learning workflows."),
            ("Integrated local payment providers", "by implementing the Payme and Click payment APIs."),
            ("Reduced repeated database operations", "by introducing Redis caching for frequently accessed data."),
            ("Created an internal administration system", "for managing thousands of majors, quizzes, and datasets."),
            ("Introduced semantic search", "by integrating Qdrant as the vector database for recommendations."),
            ("Containerized the production stack", "by deploying React and Django services with Docker."),
            ("Implemented scalable file storage", "by integrating AWS S3 for application-managed assets."),
            ("Mentored 2 Golang developers,", "guiding their work on future backend services."),
        ],
    },
    {
        "company": "EPOS Systems",
        "url": "",
        "role": "Software Engineer",
        "dates": "May 2021 – Feb 2022",
        "about": "POS platform used by 2,000+ clients for retail management and tax-related operations.",
        "bullets": [
            ("Built and maintained backend APIs used by 2,000+ clients", "with Django-based services for the POS platform."),
            ("Reached approximately 80% API test coverage", "by introducing and expanding pytest-based unit tests."),
            ("Refactored approximately 30% of the codebase", "to Clean Code and Django best practices, improving maintainability."),
            ("Reduced deployment complexity", "by containerizing multiple Django and React applications with Docker."),
            ("Centralized API documentation", "using Docusaurus and Swagger/OpenAPI."),
            ("Mentored an intern,", "providing practical backend development guidance."),
        ],
    },
]

AWARDS = [
    ("Kelajak Banki Hackathon — 1st Place", "“Kelajak Banki — Digital Solutions for the New Generation” hackathon."),
    (
        "U.S. Embassy in Tashkent — Full Scholarship",
        "Selected from ~800 applicants for the Basics of Programming for Kids program (C, algorithms, data structures).",
    ),
    ("Informatics Olympiad — 1st Place", "Awarded scholarships by 5 local universities."),
]
